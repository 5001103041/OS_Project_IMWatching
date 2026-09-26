use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai_config::{self, SETTING_CHAT_MODEL, SETTING_VISION_MODEL};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UpdateBody {
    pub vision_model: Option<String>,
    pub chat_model: Option<String>,
}

/// GET /api/ai/config — list presets + current selection.
pub async fn get_config(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<Value>> {
    Ok(Json(ai_config::config_json(&state).await))
}

/// GET /api/ai/models — live model list from each configured provider.
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<Value>> {
    Ok(Json(ai_config::available_models(&state).await))
}

/// POST /api/ai/config — update the selected models.
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<Value>> {
    if let Some(v) = body.vision_model {
        validate(&state, &v, ai_config::ModelKind::Vision)?;
        ai_config::set_setting(&state.db, SETTING_VISION_MODEL, &v).await?;
    }
    if let Some(c) = body.chat_model {
        validate(&state, &c, ai_config::ModelKind::Chat)?;
        ai_config::set_setting(&state.db, SETTING_CHAT_MODEL, &c).await?;
    }

    Ok(Json(json!({ "ok": true })))
}

fn validate(state: &AppState, id: &str, kind: ai_config::ModelKind) -> AppResult<()> {
    if id.trim().is_empty() {
        return Err(AppError::BadRequest("model id required".into()));
    }

    // Prefer an exact preset; otherwise infer the provider from the id so
    // models picked live from a provider's /models list are also allowed.
    let provider = ai_config::presets()
        .into_iter()
        .find(|p| p.id == id && p.kind == kind)
        .map(|p| p.provider)
        .or_else(|| ai_config::infer_provider(id))
        .ok_or_else(|| AppError::BadRequest(format!("unknown model: {id}")))?;

    match provider {
        ai_config::Provider::CommandCode => {
            if state.config.commandcode_base_url.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "Base URL cho provider AI mở rộng chưa được cấu hình trong .env".into(),
                ));
            }
        }
        ai_config::Provider::Gemini => {
            if state.config.ai_api_key.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "AI_API_KEY (Gemini) chưa được cấu hình trong .env".into(),
                ));
            }
        }
    }
    Ok(())
}
