//! FFmpeg/ffprobe wrappers.
//!
//! We shell out to the `ffmpeg` and `ffprobe` binaries via
//! `std::process::Command` rather than binding libav*: simpler to build,
//! debug, and upgrade. The binaries must be on PATH (or configured via
//! `FFMPEG_BIN` / `FFPROBE_BIN` env).
//!
//! FFmpeg/ffprobe write a lot to stderr (banner + progress). We redirect
//! stdio to pipes/null so the host process's stdout/stderr is not polluted
//! (which can cause shells to terminate the process tree).
//!
//! All blocking work is wrapped in `spawn_blocking` by callers.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::state::Config;

/// Metadata extracted by ffprobe.
#[derive(Debug, Clone, Default)]
pub struct ProbeMeta {
    pub duration_secs: f64,
    pub width: i64,
    pub height: i64,
    pub fps: f64,
}

#[derive(Deserialize)]
struct FfprobeJson {
    streams: Vec<FfprobeStream>,
    #[serde(default)]
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    #[serde(rename = "codec_type")]
    codec_type: String,
    width: Option<i64>,
    height: Option<i64>,
    #[serde(rename = "avg_frame_rate")]
    avg_frame_rate: Option<String>,
    #[serde(rename = "r_frame_rate")]
    r_frame_rate: Option<String>,
}

#[derive(Deserialize, Default)]
struct FfprobeFormat {
    duration: Option<String>,
}

/// Run ffprobe on a video file and return normalized metadata.
pub fn probe(config: &Config, input: &Path) -> Result<ProbeMeta> {
    let os_rt = crate::os_runtime::global();
    let _subproc_permit = os_rt
        .acquire_subprocess_permit_blocking()
        .ok_or_else(|| anyhow!("ffprobe aborted: system shutting down"))?;

    let child = Command::new(&config.ffprobe_bin)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("ffprobe: failed to spawn")?;

    let pcb_guard = os_rt.register_process(child.id(), "ffprobe", &input.display().to_string());
    let output = child
        .wait_with_output()
        .context("ffprobe: failed to wait on child process")?;
    pcb_guard.finish(output.status.code());

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json: FfprobeJson = serde_json::from_slice(&output.stdout)
        .context("ffprobe: parse json")?;

    let video_stream = json
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or_else(|| anyhow!("ffprobe: no video stream"))?;

    let duration = json
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    let fps = parse_rate(video_stream.avg_frame_rate.as_deref())
        .or_else(|| parse_rate(video_stream.r_frame_rate.as_deref()))
        .unwrap_or(0.0);

    Ok(ProbeMeta {
        duration_secs: duration,
        width: video_stream.width.unwrap_or(0),
        height: video_stream.height.unwrap_or(0),
        fps,
    })
}

fn parse_rate(s: Option<&str>) -> Option<f64> {
    let s = s?;
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

/// Transcode a source file (e.g. .webm from MediaRecorder) to H.264 .mp4.
/// `out` must end with `.mp4`. Overwrites if present.
pub fn transcode_to_mp4(config: &Config, input: &Path, out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let os_rt = crate::os_runtime::global();
    let _subproc_permit = os_rt
        .acquire_subprocess_permit_blocking()
        .ok_or_else(|| anyhow!("ffmpeg transcode aborted: system shutting down"))?;

    let child = Command::new(&config.ffmpeg_bin)
        .args([
            "-y",
            "-i",
        ])
        .arg(input)
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-an",
            "-movflags",
            "+faststart",
        ])
        .arg(out)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("ffmpeg: failed to spawn")?;

    let pcb_guard = os_rt.register_process(
        child.id(),
        "ffmpeg:transcode",
        &input.display().to_string(),
    );
    let output = child
        .wait_with_output()
        .context("ffmpeg: failed to wait on transcode process")?;
    pcb_guard.finish(output.status.code());

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg transcode failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Extract frames from a video at the configured fps into `out_dir`.
/// Files are named `frame_%05d.jpg` (1-based by FFmpeg).
/// Returns the list of produced frame paths in order.
pub fn extract_frames(config: &Config, input: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out_dir).ok();

    let fps = config.frame_fps.max(0.1);
    let vf = format!("fps={}", fps);

    let os_rt = crate::os_runtime::global();
    let _subproc_permit = os_rt
        .acquire_subprocess_permit_blocking()
        .ok_or_else(|| anyhow!("ffmpeg extract aborted: system shutting down"))?;

    let child = Command::new(&config.ffmpeg_bin)
        .args(["-y", "-i"])
        .arg(input)
        .args(["-vf", vf.as_str(), "-q:v", "3"])
        .arg(out_dir.join("frame_%05d.jpg"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("ffmpeg: failed to spawn")?;

    let pcb_guard = os_rt.register_process(
        child.id(),
        "ffmpeg:extract_frames",
        &input.display().to_string(),
    );
    let output = child
        .wait_with_output()
        .context("ffmpeg: failed to wait on extract_frames process")?;
    pcb_guard.finish(output.status.code());

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg extract frames failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Collect produced frames in order.
    let mut frames: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .context("read frames dir")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("jpg") || x.eq_ignore_ascii_case("jpeg"))
                .unwrap_or(false)
        })
        .collect();
    frames.sort();
    Ok(frames)
}

/// Build a meaningful final filename for a video based on its dominant
/// behavior and creation time.
/// Format: `<Category>_<HHMMSS_DDMMYYYY>.mp4`
pub fn build_final_filename(category: &str, when: chrono::DateTime<chrono::Local>) -> String {
    let cat = sanitize_category(category);
    format!("{}_{}.mp4", cat, when.format("%H%M%S_%d%m%Y"))
}

fn sanitize_category(s: &str) -> String {
    // Keep ASCII alphanumerics; drop accents/spaces for filename safety.
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        }
    }
    if out.is_empty() {
        "QuayCamera".to_string()
    } else {
        out
    }
}
