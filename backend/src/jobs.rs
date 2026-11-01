//! Background video processing pipeline.
//!
//! Flow (all blocking work runs in `spawn_blocking`):
//!   1. probe metadata (ffprobe)
//!   2. transcode to .mp4 if needed (ffmpeg)
//!   3. extract frames at configured fps (ffmpeg)
//!   4. for each frame: run AI analyze_frame, persist evidence + events
//!   5. compute dominant behavior, rename final file, update video row
//!
//! The pipeline is failure-tolerant per step: a failed AI on one frame does
//! not stop the whole run; the video still reaches `completed` or `failed`.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::ai::{analyze_frame, AiOutcome};
use crate::error::AppError;
use crate::models::Video;
use crate::state::AppState;
use crate::video;

const DEFAULT_CATEGORY: &str = "QuayCamera";

/// Spawn the processing pipeline for a video in the background, coordinated by
/// the OS Worker Pool Semaphore and Ready Queue (`state.os`).
pub fn spawn(state: Arc<AppState>, video_id: String) {
    tokio::spawn(async move {
        let job_guard = match state.os.acquire_pipeline_slot(&video_id).await {
            Ok(guard) => guard,
            Err(e) => {
                tracing::warn!("pipeline[{video_id}] rejected by OS scheduler: {e}");
                let _ = mark_failed(&state.db, &video_id, &e).await;
                return;
            }
        };

        match run_pipeline(&state, &video_id).await {
            Ok(()) => {
                job_guard.finish(true);
            }
            Err(e) => {
                tracing::error!("pipeline failed for video {video_id}: {e:?}");
                let _ = mark_failed(&state.db, &video_id, &e.to_string()).await;
                job_guard.finish(false);
            }
        }
    });
}

async fn run_pipeline(state: &AppState, video_id: &str) -> Result<(), AppError> {
    // Load video row.
    let mut video = sqlx::query_as::<_, Video>("SELECT * FROM videos WHERE id = ?")
        .bind(video_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Invalidate any cached AI summary — the video is being reprocessed so the
    // old summary is stale. A fresh one will be generated on the next chat request.
    sqlx::query(
        "UPDATE videos SET ai_summary = NULL, ai_summary_generated_at = NULL, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(video_id)
    .execute(&state.db)
    .await?;

    set_status(&state.db, video_id, "transcoding").await?;
    tracing::info!("pipeline[{}]: start", video_id);

    let media_dir = state.config.media_dir.clone();
    let config = state.config.clone();

    // --- 1. probe ---
    let raw_path = media_dir.join(&video.storage_path);
    let probe_meta = tokio::task::spawn_blocking({
        let cfg = config.clone();
        let p = raw_path.clone();
        move || video::probe(&cfg, &p)
    })
    .await
    .map_err(|e| AppError::Internal(format!("join probe: {e}")))??;

    // Persist duration/dimensions.
    sqlx::query(
        "UPDATE videos SET duration_secs = ?, width = ?, height = ?, fps = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(probe_meta.duration_secs)
    .bind(probe_meta.width)
    .bind(probe_meta.height)
    .bind(probe_meta.fps)
    .bind(video_id)
    .execute(&state.db)
    .await?;

    // --- 2. transcode to .mp4 if source isn't .mp4 ---
    let final_path = if video.storage_path.ends_with(".mp4") {
        raw_path.clone()
    } else {
        let mp4_name = video
            .final_filename
            .clone()
            .unwrap_or_else(|| format!("{}.mp4", video_id));
        let mp4_rel = format!("videos/{}/{}", video_id, mp4_name);
        let mp4_abs = media_dir.join(&mp4_rel);
        tokio::task::spawn_blocking({
            let cfg = config.clone();
            let i = raw_path.clone();
            let o = mp4_abs.clone();
            move || video::transcode_to_mp4(&cfg, &i, &o)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join transcode: {e}")))??;

        // Update storage_path to the transcoded file.
        sqlx::query("UPDATE videos SET storage_path = ?, mime_type = 'video/mp4', updated_at = datetime('now') WHERE id = ?")
            .bind(&mp4_rel)
            .bind(video_id)
            .execute(&state.db)
            .await?;
        video.storage_path = mp4_rel;
        mp4_abs
    };

    // --- 3. extract frames ---
    set_status(&state.db, video_id, "extracting").await?;
    let frames_dir = media_dir.join(format!("videos/{}/frames", video_id));
    let frame_paths = tokio::task::spawn_blocking({
        let cfg = config.clone();
        let i = final_path.clone();
        let d = frames_dir.clone();
        move || video::extract_frames(&cfg, &i, &d)
    })
    .await
    .map_err(|e| AppError::Internal(format!("join extract: {e}")))??;

    tracing::info!(
        "pipeline[{}]: extracted {} frames",
        video_id,
        frame_paths.len()
    );

    // If the original upload (e.g. MediaRecorder webm) had no duration header,
    // probe the transcoded mp4 or estimate duration from extracted frames.
    if probe_meta.duration_secs <= 0.0 {
        let post_probe = tokio::task::spawn_blocking({
            let cfg = config.clone();
            let p = final_path.clone();
            move || video::probe(&cfg, &p)
        })
        .await
        .ok()
        .and_then(|r| r.ok());

        let final_dur = post_probe
            .map(|p| p.duration_secs)
            .filter(|&d| d > 0.0)
            .unwrap_or_else(|| {
                let sample_fps = config.frame_fps.max(0.1);
                frame_paths.len() as f64 / sample_fps
            });

        if final_dur > 0.0 {
            let _ = sqlx::query(
                "UPDATE videos SET duration_secs = ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(final_dur)
            .bind(video_id)
            .execute(&state.db)
            .await;
        }
    }

    // --- 4. analyze each frame ---
    set_status(&state.db, video_id, "analyzing").await?;
    let fps = config.frame_fps.max(0.1);
    let frame_interval = 1.0 / fps;
    let mut behavior_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    // Pre-insert all extracted frames with analyzed = 0 so progress polling accurately reflects real progress
    let _ = sqlx::query("DELETE FROM video_frames WHERE video_id = ?")
        .bind(video_id)
        .execute(&state.db)
        .await;

    for (idx, _) in frame_paths.iter().enumerate() {
        let frame_index = (idx + 1) as i64;
        let timestamp_secs = (idx as f64) / fps;
        let frame_rel = format!("videos/{}/frames/frame_{:05}.jpg", video_id, frame_index);
        let fid = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO video_frames (id, video_id, frame_index, timestamp_secs, storage_path, width, height, analyzed)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&fid)
        .bind(video_id)
        .bind(frame_index)
        .bind(timestamp_secs)
        .bind(&frame_rel)
        .bind(probe_meta.width)
        .bind(probe_meta.height)
        .execute(&state.db)
        .await;
    }

    // Motion detection: compare consecutive frames to tell "walking" (movement)
    // from "standing" (still) — a single static frame cannot distinguish them.
    let mut prev_jpeg: Option<Vec<u8>> = None;
    let mut current: Option<Segment> = None;

    for (idx, frame_abs) in frame_paths.iter().enumerate() {
        if state.os.is_shutting_down() {
            tracing::warn!("pipeline[{}]: interrupted by OS shutdown signal at frame {}", video_id, idx + 1);
            return Err(AppError::Internal("Pipeline interrupted by OS shutdown signal".into()));
        }

        let frame_index = (idx + 1) as i64; // FFmpeg is 1-based
        let timestamp_secs = (idx as f64) / fps;
        let end_ts = timestamp_secs + frame_interval;

        let frame_rel = format!("videos/{}/frames/frame_{:05}.jpg", video_id, frame_index);

        // Fetch or ensure frame row, mark analyzed = 1
        let frame_id = match sqlx::query_scalar::<_, String>(
            "SELECT id FROM video_frames WHERE video_id = ? AND frame_index = ?",
        )
        .bind(video_id)
        .bind(frame_index)
        .fetch_optional(&state.db)
        .await?
        {
            Some(id) => {
                let _ = sqlx::query("UPDATE video_frames SET analyzed = 1 WHERE id = ?")
                    .bind(&id)
                    .execute(&state.db)
                    .await;
                id
            }
            None => {
                let fid = uuid::Uuid::new_v4().to_string();
                let _ = sqlx::query(
                    "INSERT INTO video_frames (id, video_id, frame_index, timestamp_secs, storage_path, width, height, analyzed)
                     VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
                )
                .bind(&fid)
                .bind(video_id)
                .bind(frame_index)
                .bind(timestamp_secs)
                .bind(&frame_rel)
                .bind(probe_meta.width)
                .bind(probe_meta.height)
                .execute(&state.db)
                .await;
                fid
            }
        };

        // Read frame bytes for AI + motion.
        let jpeg = match tokio::fs::read(frame_abs).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("pipeline[{}]: read frame {idx}: {e}", video_id);
                continue;
            }
        };

        // Motion score vs previous frame (0..1).
        let motion = match &prev_jpeg {
            Some(prev) => motion_score(prev, &jpeg),
            None => 0.0,
        };
        prev_jpeg = Some(jpeg.clone());

        let outcome = analyze_frame(state, &jpeg).await;

        // Throttle between frames (lightweight 300ms delay to prevent rate-limiting while keeping pipeline fast).
        if state.config.ai_enabled && idx + 1 < frame_paths.len() {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        match outcome {
            AiOutcome::Ok(verdict) => {
                if let Some(scene) = &verdict.scene {
                    let scene_str = serde_json::to_string(scene).unwrap_or_default();
                    sqlx::query("UPDATE video_frames SET scene_json = ? WHERE id = ?")
                        .bind(&scene_str)
                        .bind(&frame_id)
                        .execute(&state.db)
                        .await
                        .ok(); // best-effort, don't fail pipeline
                }
                for b in verdict.behaviors {
                    let mut behavior_name = b.behavior_name.trim().to_lowercase();

                    // Motion-aware locomotion: a static "standing" frame that
                    // actually moved becomes "walking"; a still frame stays
                    // "standing". Only affects these two labels.
                    if behavior_name == "standing" || behavior_name == "walking" {
                        behavior_name = if motion >= 0.025 {
                            "walking".to_string()
                        } else {
                            "standing".to_string()
                        };
                    }

                    // Inherent normal behaviors that must NEVER be classified as dangerous:
                    let is_inherently_normal = matches!(
                        behavior_name.as_str(),
                        "walking"
                            | "standing"
                            | "sitting"
                            | "studying"
                            | "talking"
                            | "gesturing"
                            | "gesture"
                            | "pointing"
                            | "thumbs_up"
                            | "phone_use"
                            | "carrying_object"
                            | "reading"
                            | "eating"
                            | "drinking"
                            | "dancing"
                            | "hugging"
                            | "driving"
                            | "working"
                            | "exercising"
                    );

                    let severity = if is_inherently_normal {
                        "normal"
                    } else if b.severity.eq_ignore_ascii_case("dangerous") {
                        "dangerous"
                    } else {
                        "normal"
                    };

                    let confidence = b.confidence.max(
                        b.subjects.first().map(|s| s.confidence).unwrap_or(0.0),
                    );

                    let seg = Segment {
                        behavior_name: behavior_name.clone(),
                        severity: severity.to_string(),
                        subjects: b.subjects.clone(),
                        confidence,
                        interaction_type: b.interaction_type.clone(),
                        reason: b.reason.clone(),
                        start_ts: timestamp_secs,
                        end_ts,
                        frame_id: frame_id.clone(),
                        frame_rel: frame_rel.clone(),
                    };

                    // Merge with the open segment if it's the same behavior AND
                    // the same subjects (so two different people doing the same
                    // action aren't collapsed into one event).
                    if let Some(open) = current.as_mut() {
                        let same_behavior = open.behavior_name == seg.behavior_name
                            && open.severity == seg.severity;
                        let same_subjects = subject_signature(&open.subjects)
                            == subject_signature(&seg.subjects);
                        if same_behavior && same_subjects {
                            // Extend the time range.
                            open.end_ts = end_ts;
                            // Merge unseen subjects (e.g. a new person entered).
                            for s in &seg.subjects {
                                let known = open.subjects.iter().any(|o| {
                                    o.gender == s.gender && o.clothing_color == s.clothing_color
                                });
                                if !known {
                                    open.subjects.push(s.clone());
                                }
                            }
                            // Keep the higher-confidence reason.
                            if seg.confidence > open.confidence {
                                open.confidence = seg.confidence;
                                open.reason = seg.reason.clone();
                            }
                            continue;
                        } else {
                            let done = current.take().unwrap();
                            insert_segment(state, video_id, &done, &mut behavior_counts).await?;
                        }
                    }
                    current = Some(seg);
                }
            }
            AiOutcome::Failed => {
                tracing::warn!("pipeline[{}]: AI failed on frame {idx}", video_id);
                // Flush any open segment so a later behavior starts fresh.
                if let Some(done) = current.take() {
                    insert_segment(state, video_id, &done, &mut behavior_counts).await?;
                }
            }
            AiOutcome::Skipped => {
                // AI disabled: store a plain "unanalyzed" event.
                let event_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO detected_events
                        (id, video_id, frame_id, timestamp_secs, end_timestamp_secs, behavior_name, severity,
                         gender, clothing_color, confidence, ai_reason, frame_image_path)
                     VALUES (?, ?, ?, ?, ?, 'unanalyzed', 'normal', 'unknown', NULL, 0.0, 'AI disabled', ?)",
                )
                .bind(&event_id)
                .bind(video_id)
                .bind(&frame_id)
                .bind(timestamp_secs)
                .bind(end_ts)
                .bind(&frame_rel)
                .execute(&state.db)
                .await?;
            }
        }
    }

    // Flush the final open segment.
    if let Some(done) = current.take() {
        insert_segment(state, video_id, &done, &mut behavior_counts).await?;
    }

    // --- 5. compute dominant behavior, rename final file ---
    let dominant = behavior_counts
        .iter()
        .max_by_key(|(_, &c)| c)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| DEFAULT_CATEGORY.to_string());

    let now = chrono::Local::now();
    let new_final_name = video::build_final_filename(&dominant, now);
    let new_rel = format!("videos/{}/{}", video_id, new_final_name);
    let new_abs = media_dir.join(&new_rel);

    // Rename the transcoded/orig file on disk if it differs.
    if new_abs != final_path {
        if let Some(parent) = new_abs.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        // Best-effort rename; ignore errors (file may be in use).
        let _ = tokio::fs::rename(&final_path, &new_abs).await;
    }

    let fallback_dur = if !frame_paths.is_empty() {
        frame_paths.len() as f64 / fps
    } else {
        0.0
    };

    sqlx::query(
        "UPDATE videos SET final_filename = ?, behavior_category = ?, storage_path = ?, status = 'completed', error_message = NULL, duration_secs = CASE WHEN (duration_secs IS NULL OR duration_secs <= 0) THEN ? ELSE duration_secs END, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&new_final_name)
    .bind(&dominant)
    .bind(&new_rel)
    .bind(fallback_dur)
    .bind(video_id)
    .execute(&state.db)
    .await?;

    tracing::info!("pipeline[{}]: completed (dominant={})", video_id, dominant);
    Ok(())
}

async fn set_status(db: &SqlitePool, video_id: &str, status: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE videos SET status = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(status)
        .bind(video_id)
        .execute(db)
        .await?;
    Ok(())
}

/// A contiguous run of the same behavior across consecutive frames.
struct Segment {
    behavior_name: String,
    severity: String,
    subjects: Vec<crate::ai::AiSubject>,
    confidence: f32,
    interaction_type: Option<String>,
    reason: String,
    start_ts: f64,
    end_ts: f64,
    frame_id: String,
    frame_rel: String,
}

/// Persist a merged segment as one detected_events row, then save any extended
/// attributes (clothing_type, glasses, hat, bag, age_group, mask, hair_color)
/// into the `event_attributes` EAV table.
async fn insert_segment(
    state: &AppState,
    video_id: &str,
    seg: &Segment,
    behavior_counts: &mut std::collections::HashMap<String, u32>,
) -> Result<(), AppError> {
    // First subject -> denormalized search columns.
    let (gender, clothing_color) = seg
        .subjects
        .first()
        .map(|s| {
            (
                s.gender.trim().to_lowercase(),
                if s.clothing_color.trim().is_empty() {
                    None
                } else {
                    Some(s.clothing_color.trim().to_lowercase())
                },
            )
        })
        .unwrap_or(("unknown".to_string(), None));

    let subjects_json = serde_json::to_string(&seg.subjects).unwrap_or_else(|_| "[]".to_string());
    let event_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO detected_events
            (id, video_id, frame_id, timestamp_secs, end_timestamp_secs,
             behavior_name, severity, gender, clothing_color, confidence,
             ai_reason, subjects_json, frame_image_path)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event_id)
    .bind(video_id)
    .bind(&seg.frame_id)
    .bind(seg.start_ts)
    .bind(seg.end_ts)
    .bind(&seg.behavior_name)
    .bind(&seg.severity)
    .bind(&gender)
    .bind(&clothing_color)
    .bind(seg.confidence as f64)
    .bind(&seg.reason)
    .bind(&subjects_json)
    .bind(&seg.frame_rel)
    .execute(&state.db)
    .await?;

    // Persist extended attributes for each subject into the EAV table.
    // We prefix subject_index (s0_, s1_, ...) so attributes from different
    // people in the same event are distinguishable.
    
    if let Some(itype) = &seg.interaction_type {
        let attr_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO event_attributes (id, event_id, attribute_type, attribute_value, confidence)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&attr_id)
        .bind(&event_id)
        .bind("interaction_type")
        .bind(itype)
        .bind(seg.confidence as f64)
        .execute(&state.db)
        .await?;
    }

    for (subj_idx, subject) in seg.subjects.iter().enumerate() {
        let prefix = format!("s{subj_idx}_");
        
        if let Some(role) = &subject.role {
            let attr_id = uuid::Uuid::new_v4().to_string();
            let attr_type = format!("{}role", prefix);
            sqlx::query(
                "INSERT INTO event_attributes (id, event_id, attribute_type, attribute_value, confidence)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&attr_id)
            .bind(&event_id)
            .bind(&attr_type)
            .bind(role)
            .bind(subject.confidence as f64)
            .execute(&state.db)
            .await?;
        }

        for (attr_type_raw, attr_value) in subject.extended_attrs() {
            let attr_id = uuid::Uuid::new_v4().to_string();
            let attr_type = format!("{prefix}{attr_type_raw}");
            sqlx::query(
                "INSERT INTO event_attributes (id, event_id, attribute_type, attribute_value, confidence)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&attr_id)
            .bind(&event_id)
            .bind(&attr_type)
            .bind(&attr_value)
            .bind(subject.confidence as f64)
            .execute(&state.db)
            .await?;
        }
    }

    *behavior_counts.entry(seg.behavior_name.clone()).or_insert(0) += 1;
    Ok(())
}

/// Compute a rough motion score between two JPEG frames by comparing decoded
/// luminance at a downsampled grid. Returns 0..1 (fraction of cells that
/// changed meaningfully). Best-effort: any decode error yields 0.
fn motion_score(prev: &[u8], next: &[u8]) -> f64 {
    use image::GenericImageView;

    let decode = |bytes: &[u8]| -> Option<image::DynamicImage> {
        image::load_from_memory(bytes).ok()
    };

    let (Some(a), Some(b)) = (decode(prev), decode(next)) else {
        return 0.0;
    };

    let (wa, ha) = a.dimensions();
    let (wb, hb) = b.dimensions();
    if wa == 0 || ha == 0 || wb == 0 || hb == 0 {
        return 0.0;
    }

    // Downsample to a coarse grid (max 24x24 cells) for speed.
    let cells = 24u32;
    let step_x = (wa.min(wb) / cells).max(1);
    let step_y = (ha.min(hb) / cells).max(1);

    let mut changed = 0u32;
    let mut total = 0u32;

    let mut y = 0;
    while y < ha.min(hb) {
        let mut x = 0;
        while x < wa.min(wb) {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            let lum_a = luminance(pa.0);
            let lum_b = luminance(pb.0);
            let diff = (lum_a as i32 - lum_b as i32).abs();
            if diff > 25 {
                changed += 1;
            }
            total += 1;
            x += step_x;
        }
        y += step_y;
    }

    if total == 0 {
        0.0
    } else {
        changed as f64 / total as f64
    }
}

#[inline]
fn luminance(rgb: [u8; 4]) -> u8 {
    // Simple Rec.601 luma.
    let r = rgb[0] as u32;
    let g = rgb[1] as u32;
    let b = rgb[2] as u32;
    ((r * 299 + g * 587 + b * 114) / 1000) as u8
}

/// A stable, order-independent signature of a subject set, used to decide
/// whether two frames show the *same* people (merge) or different people.
fn subject_signature(subjects: &[crate::ai::AiSubject]) -> Vec<String> {
    let mut sig: Vec<String> = subjects
        .iter()
        .map(|s| format!("{}|{}", s.gender.trim().to_lowercase(), s.clothing_color.trim().to_lowercase()))
        .collect();
    sig.sort();
    sig.dedup();
    sig
}

async fn mark_failed(db: &SqlitePool, video_id: &str, msg: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE videos SET status = 'failed', error_message = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(msg)
        .bind(video_id)
        .execute(db)
        .await?;
    Ok(())
}
