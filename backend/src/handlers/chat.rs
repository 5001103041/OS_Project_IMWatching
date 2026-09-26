use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::chatbot;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChatBody {
    pub question: String,
}

/// POST /api/chat/videos/:id
/// Answer a question using the cached video summary as context.
pub async fn ask(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(video_id): Path<String>,
    Json(body): Json<ChatBody>,
) -> AppResult<Json<Value>> {
    if body.question.trim().is_empty() {
        return Err(AppError::BadRequest("question required".into()));
    }
    let answer = chatbot::answer(&state, &auth.0.id, &video_id, &body.question).await?;
    Ok(Json(json!({ "answer": answer })))
}

/// GET /api/chat/videos/:id/summary
/// Return the cached holistic summary, generating it on first call (lazy).
pub async fn summary(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(video_id): Path<String>,
) -> AppResult<Json<Value>> {
    let summary = chatbot::get_or_generate_summary(&state, &auth.0.id, &video_id).await?;
    Ok(Json(json!({ "summary": summary, "cached": true })))
}

/// POST /api/chat/videos/:id/summary
/// Force-regenerate the summary (e.g. after manual event corrections).
pub async fn regenerate_summary(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(video_id): Path<String>,
) -> AppResult<Json<Value>> {
    // Wipe cache first so get_or_generate_summary always calls summarize().
    sqlx::query(
        "UPDATE videos SET ai_summary = NULL, ai_summary_generated_at = NULL \
         WHERE id = ? AND owner_id = ?",
    )
    .bind(&video_id)
    .bind(&auth.0.id)
    .execute(&state.db)
    .await?;

    let summary = chatbot::get_or_generate_summary(&state, &auth.0.id, &video_id).await?;
    Ok(Json(json!({ "summary": summary, "cached": false })))
}
