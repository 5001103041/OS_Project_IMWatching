use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::header::{self, HeaderMap};
use axum::http::{StatusCode};
use axum::response::Response;
use axum::body::Body;
use serde::Deserialize;

use crate::auth::{decode_access_token, AuthUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Optional `?token=` auth for <video>/<img> tags, which cannot send an
/// Authorization header.
#[derive(Deserialize, Default)]
pub struct MediaQuery {
    pub token: Option<String>,
}

/// Resolve the caller's user id from either the Bearer header or `?token=`.
async fn resolve_user(state: &AppState, header: Option<AuthUser>, q: &MediaQuery) -> AppResult<String> {
    if let Some(AuthUser(u)) = header {
        return Ok(u.id);
    }
    if let Some(token) = &q.token {
        let claims = decode_access_token(state, token)?;
        return Ok(claims.sub);
    }
    Err(AppError::Unauthorized)
}

/// Serve a video file with HTTP Range support so the browser player can seek.
/// GET /api/media/videos/:id/stream?token=...
pub async fn stream_video(
    State(state): State<Arc<AppState>>,
    auth: Option<AuthUser>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<MediaQuery>,
) -> AppResult<Response> {
    let user_id = resolve_user(&state, auth, &q).await?;

    // Only the owner can stream.
    let storage_path: Option<String> =
        sqlx::query_scalar("SELECT storage_path FROM videos WHERE id = ? AND owner_id = ?")
            .bind(&id)
            .bind(&user_id)
            .fetch_optional(&state.db)
            .await?;
    let Some(rel) = storage_path else {
        return Err(AppError::NotFound);
    };

    let abs = resolve_safe(&state, &rel)?;
    let file_len = tokio::fs::metadata(&abs)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .len();

    let mime = mime_guess::from_path(&rel).first_or_octet_stream().to_string();

    // Parse Range header: "bytes=start-end"
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, file_len) {
            let length = end - start + 1;
            let bytes = read_range(&abs, start, length)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            return Ok(Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_len))
                .header(header::CONTENT_LENGTH, length)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "private, max-age=3600")
                .body(Body::from(bytes))
                .unwrap());
        }
    }

    // Full file.
    let bytes = tokio::fs::read(&abs)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, file_len)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .status(StatusCode::OK)
        .body(Body::from(bytes))
        .unwrap())
}

/// Parse a `bytes=start-end` Range header. Returns inclusive (start, end).
fn parse_range(range: &str, file_len: u64) -> Option<(u64, u64)> {
    let spec = range.strip_prefix("bytes=")?;
    let (start_s, end_s) = spec.split_once('-')?;
    let start: u64 = start_s.parse().ok()?;
    if start >= file_len {
        return None;
    }
    let end = if end_s.is_empty() {
        file_len - 1
    } else {
        end_s.parse::<u64>().ok()?.min(file_len - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

async fn read_range(path: &std::path::Path, start: u64, length: u64) -> Result<Vec<u8>, std::io::Error> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut f = tokio::fs::File::open(path).await?;
    f.seek(std::io::SeekFrom::Start(start)).await?;
    let mut buf = vec![0u8; length as usize];
    f.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Serve a single frame image (evidence).
/// GET /api/media/frames/:video_id/:index?token=...
pub async fn serve_frame(
    State(state): State<Arc<AppState>>,
    auth: Option<AuthUser>,
    Path((video_id, index)): Path<(String, String)>,
    Query(q): Query<MediaQuery>,
) -> AppResult<Response> {
    let user_id = resolve_user(&state, auth, &q).await?;

    // Owner check via video.
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM videos WHERE id = ?")
        .bind(&video_id)
        .fetch_optional(&state.db)
        .await?;
    if owner.as_deref() != Some(user_id.as_str()) {
        return Err(AppError::NotFound);
    }

    // Map index -> storage path. Support "frame_00042" or plain number.
    let rel = if let Ok(n) = index.parse::<u32>() {
        format!("videos/{}/frames/frame_{:05}.jpg", video_id, n)
    } else {
        format!("videos/{}/frames/{}", video_id, index)
    };

    let abs = resolve_safe(&state, &rel)?;
    let bytes = tokio::fs::read(&abs)
        .await
        .map_err(|_| AppError::NotFound)?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(bytes))
        .unwrap())
}

fn resolve_safe(state: &AppState, rel: &str) -> AppResult<std::path::PathBuf> {
    let abs = state.config.media_dir.join(rel);
    // Defense-in-depth against path traversal.
    if !abs.starts_with(&state.config.media_dir) {
        return Err(AppError::Forbidden);
    }
    Ok(abs)
}
