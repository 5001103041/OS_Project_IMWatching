use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::jobs;
use crate::models::Video;
use crate::state::AppState;

const DEFAULT_CATEGORY: &str = "QuayCamera";

/// POST /api/videos — upload a video (multipart field "file").
pub async fn upload(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                original_filename = field.file_name().map(|s| s.to_string());
                content_type = field.content_type().map(|s| s.to_string());
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                file_bytes = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    let bytes = file_bytes.ok_or_else(|| AppError::BadRequest("file required".into()))?;
    let original_filename =
        original_filename.unwrap_or_else(|| "upload.webm".to_string());

    if bytes.len() > state.config.max_upload_bytes {
        return Err(AppError::BadRequest("file too large".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let rel_dir = format!("videos/{}", id);

    // Name the file per convention: <Category>_<HHMMSS_DDMMYYYY>.mp4
    // At upload time we don't know the final category yet, so use DEFAULT_CATEGORY.
    let now = chrono::Local::now();
    let final_filename = format!(
        "{}_{}.mp4",
        DEFAULT_CATEGORY,
        now.format("%H%M%S_%d%m%Y")
    );

    let storage_path = format!("{}/{}", rel_dir, final_filename);
    let abs = state.config.media_dir.join(&storage_path);
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    tokio::fs::write(&abs, &bytes)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mime = content_type
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| mime_guess::from_path(&original_filename).first_or_octet_stream().to_string());

    sqlx::query(
        "INSERT INTO videos (id, owner_id, original_filename, final_filename, storage_path, mime_type, file_size_bytes, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'uploaded')",
    )
    .bind(&id)
    .bind(&auth.0.id)
    .bind(&original_filename)
    .bind(&final_filename)
    .bind(&storage_path)
    .bind(&mime)
    .bind(bytes.len() as i64)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "id": id,
        "final_filename": final_filename,
        "status": "uploaded",
    })))
}

/// GET /api/videos — list the caller's videos, newest first.
pub async fn list(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<Value>> {
    let rows = sqlx::query_as::<_, Video>(
        "SELECT * FROM videos WHERE owner_id = ? ORDER BY created_at DESC LIMIT 200",
    )
    .bind(&auth.0.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({ "videos": rows.iter().map(video_json).collect::<Vec<_>>() })))
}

/// POST /api/videos/:id/process — kick off the background analysis pipeline.
pub async fn process(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    // Ensure the caller owns the video.
    let _ = fetch_owned(&state, &auth.0.id, &id).await?;
    jobs::spawn(state.clone(), id.clone());
    Ok(Json(json!({ "id": id, "status": "processing" })))
}

/// GET /api/videos/:id — single video.
pub async fn get(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let video = fetch_owned(&state, &auth.0.id, &id).await?;
    Ok(Json(video_json(&video)))
}

async fn fetch_owned(state: &AppState, owner_id: &str, id: &str) -> AppResult<Video> {
    let mut video = sqlx::query_as::<_, Video>("SELECT * FROM videos WHERE id = ? AND owner_id = ?")
        .bind(id)
        .bind(owner_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // If duration is missing or 0, backfill from detected_events if available
    if video.duration_secs.unwrap_or(0.0) <= 0.0 {
        let max_event_ts: Option<f64> = sqlx::query_scalar(
            "SELECT MAX(COALESCE(end_timestamp_secs, timestamp_secs)) FROM detected_events WHERE video_id = ?"
        )
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(None);

        if let Some(ts) = max_event_ts {
            if ts > 0.0 {
                let dur = ts + 1.0;
                let _ = sqlx::query(
                    "UPDATE videos SET duration_secs = ?, updated_at = datetime('now') WHERE id = ?",
                )
                .bind(dur)
                .bind(id)
                .execute(&state.db)
                .await;
                video.duration_secs = Some(dur);
            }
        }
    }

    Ok(video)
}

/// DELETE /api/videos/:id — delete a video (row + files), owner only.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let video = fetch_owned(&state, &auth.0.id, &id).await?;

    // Delete the whole media directory for this video (raw + frames).
    let dir = state.config.media_dir.join(format!("videos/{}", id));
    let _ = tokio::fs::remove_dir_all(&dir).await;

    // DB row removal cascades to frames/events/attributes/chat via FK.
    sqlx::query("DELETE FROM videos WHERE id = ? AND owner_id = ?")
        .bind(&id)
        .bind(&auth.0.id)
        .execute(&state.db)
        .await?;

    tracing::info!("deleted video {id} ({}), owner {}", video.original_filename, auth.0.id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/videos/import-library — import all sample videos under
/// `MEDIA_DIR/vid_lib` into the caller's library. Returns import count.
pub async fn import_library(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<Value>> {
    let lib_dir = state.config.media_dir.join("vid_lib");
    let entries: Vec<std::path::PathBuf> = match tokio::fs::read_dir(&lib_dir).await {
        Ok(mut rd) => {
            let mut v = Vec::new();
            while let Some(e) = rd.next_entry().await.map_err(|e| AppError::Internal(e.to_string()))? {
                let p = e.path();
                if p.is_file() {
                    v.push(p);
                }
            }
            v.sort();
            v
        }
        Err(_) => {
            return Ok(Json(json!({ "imported": 0, "message": "vid_lib not found" })));
        }
    };

    let mut imported = 0;
    for src in entries {
        let Some(fname) = src.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()) else {
            continue;
        };
        // Skip if already imported (same original_filename).
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM videos WHERE owner_id = ? AND original_filename = ? LIMIT 1",
        )
        .bind(&auth.0.id)
        .bind(&fname)
        .fetch_optional(&state.db)
        .await?;
        if exists.is_some() {
            continue;
        }

        let bytes = match tokio::fs::read(&src).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("import: read {fname}: {e}");
                continue;
            }
        };

        let id = uuid::Uuid::new_v4().to_string();
        let rel_dir = format!("videos/{}", id);
        let now = chrono::Local::now();
        let final_filename = format!("{}_{}.mp4", DEFAULT_CATEGORY, now.format("%H%M%S_%d%m%Y"));
        let storage_path = format!("{}/{}/{}", "videos", id, fname);
        let abs = state.config.media_dir.join(&storage_path);
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::Internal(e.to_string()))?;
        }
        tokio::fs::write(&abs, &bytes).await.map_err(|e| AppError::Internal(e.to_string()))?;

        let mime = mime_guess::from_path(&fname).first_or_octet_stream().to_string();
        let _ = rel_dir;
        let _ = final_filename;
        sqlx::query(
            "INSERT INTO videos (id, owner_id, original_filename, final_filename, storage_path, mime_type, file_size_bytes, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'uploaded')",
        )
        .bind(&id)
        .bind(&auth.0.id)
        .bind(&fname)
        .bind(&fname)
        .bind(&storage_path)
        .bind(&mime)
        .bind(bytes.len() as i64)
        .execute(&state.db)
        .await?;

        imported += 1;
    }

    Ok(Json(json!({ "imported": imported })))
}

fn video_json(v: &Video) -> Value {
    json!({
        "id": v.id,
        "title": v.title,
        "original_filename": v.original_filename,
        "final_filename": v.final_filename,
        "behavior_category": v.behavior_category,
        "mime_type": v.mime_type,
        "duration_secs": v.duration_secs,
        "width": v.width,
        "height": v.height,
        "fps": v.fps,
        "file_size_bytes": v.file_size_bytes,
        "status": v.status,
        "error_message": v.error_message,
        "created_at": v.created_at,
        "updated_at": v.updated_at,
    })
}

/// GET /api/videos/:id/progress
pub async fn progress(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(video_id): Path<String>,
) -> AppResult<Json<Value>> {
    // Verify ownership
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM videos WHERE id = ?")
        .bind(&video_id)
        .fetch_optional(&state.db)
        .await?;
    if owner.as_deref() != Some(&auth.0.id) {
        return Err(AppError::NotFound);
    }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM video_frames WHERE video_id = ?")
        .bind(&video_id)
        .fetch_one(&state.db)
        .await?;
    let analyzed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM video_frames WHERE video_id = ? AND analyzed = 1")
        .bind(&video_id)
        .fetch_one(&state.db)
        .await?;
    let status: String = sqlx::query_scalar("SELECT status FROM videos WHERE id = ?")
        .bind(&video_id)
        .fetch_one(&state.db)
        .await?;

    let progress_pct = match status.as_str() {
        "uploaded" => 0,
        "transcoding" => 5,
        "extracting" => 15,
        "completed" => 100,
        "failed" => 0,
        _ => {
            if total > 0 {
                let ratio = (analyzed as f64 / total as f64).clamp(0.0, 1.0);
                (15.0 + ratio * 84.0).round() as u32
            } else {
                15
            }
        }
    };

    Ok(Json(json!({
        "total_frames": total,
        "analyzed_frames": analyzed,
        "status": status,
        "progress_pct": progress_pct,
    })))
}
