//! AI model registry + runtime selection.
//!
//! Users pick a vision model and a chat model in the UI. The choice is stored
//! in the `settings` table (key: `vision_model`, `chat_model`) and resolved
//! here at call time, falling back to env defaults when unset.
//!
//! Each preset carries a `provider` ("gemini" | "commandcode") so we can build
//! the right endpoint + key for it. The actual endpoint/key strings come from
//! the corresponding env vars.

use serde::Deserialize;
use sqlx::SqlitePool;

use crate::state::AppState;

pub const SETTING_VISION_MODEL: &str = "vision_model";
pub const SETTING_CHAT_MODEL: &str = "chat_model";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Gemini,
    CommandCode,
}

/// A selectable model preset.
#[derive(Debug, Clone)]
pub struct ModelPreset {
    pub id: String,           // the model id to send to the API
    pub label: String,        // human-friendly name
    pub provider: Provider,
    pub kind: ModelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Vision,
    Chat,
}

/// Registry of selectable models. Users can choose any of these in the UI.
/// Vision models MUST be multimodal; chat models may be text-only.
/// IDs verified against each provider's live `/models` endpoint.
pub fn presets() -> Vec<ModelPreset> {
    vec![
        // ---- Vision (multimodal) — Gemini (Google) ----
        ModelPreset {
            id: "gemini-3.8-flash".into(),
            label: "Gemini 3.8 Flash (Google · Mới nhất & Toàn diện)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-3.7-flash".into(),
            label: "Gemini 3.7 Flash (Google · Đa năng)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-3.6-flash".into(),
            label: "Gemini 3.6 Flash (Google)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-3.5-flash".into(),
            label: "Gemini 3.5 Flash (Google)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-3.5-flash-lite".into(),
            label: "Gemini 3.5 Flash Lite (Google · Siêu rẻ & nhanh)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-3.1-flash-lite".into(),
            label: "Gemini 3.1 Flash Lite (Google · Siêu rẻ)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-2.5-flash".into(),
            label: "Gemini 2.5 Flash (Google)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "gemini-2.5-flash-lite".into(),
            label: "Gemini 2.5 Flash Lite (Google · Tiết kiệm)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Vision,
        },
        // ---- Vision (multimodal) — Qwen (Alibaba) ----
        ModelPreset {
            id: "Qwen/Qwen3.7-Flash".into(),
            label: "Qwen 3.7 Flash Vision (Alibaba · Tiết kiệm & nhanh)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "Qwen/Qwen3.8-Omni-Flash".into(),
            label: "Qwen 3.8 Omni Flash Vision (Alibaba · Đa phương thức)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Vision,
        },
        ModelPreset {
            id: "deepseek/deepseek-v4-flash-vision-exp".into(),
            label: "DeepSeek V4 Flash Vision (DeepSeek · Tối ưu)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Vision,
        },
        // ---- Chat (text) — Gemini, Qwen, DeepSeek & Ling ----
        ModelPreset {
            id: "gemini-3.8-flash".into(),
            label: "Gemini 3.8 Flash (Google · Thông minh nhất)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "gemini-3.5-flash-lite".into(),
            label: "Gemini 3.5 Flash Lite (Google · Siêu rẻ)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "gemini-3.1-flash-lite".into(),
            label: "Gemini 3.1 Flash Lite (Google · Rẻ nhất)".into(),
            provider: Provider::Gemini,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "Qwen/Qwen3.8-Flash".into(),
            label: "Qwen 3.8 Flash (Alibaba · Phản hồi nhanh)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "Qwen/Qwen3.7-Flash".into(),
            label: "Qwen 3.7 Flash (Alibaba · Tiết kiệm)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "deepseek/deepseek-v4-flash".into(),
            label: "DeepSeek V4 Flash (DeepSeek · Siêu rẻ)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Chat,
        },
        ModelPreset {
            id: "inclusionai/ling-3.0-flash-sante:free".into(),
            label: "Ling 3.0 Flash (InclusionAI · Miễn phí)".into(),
            provider: Provider::CommandCode,
            kind: ModelKind::Chat,
        },
    ]
}

/// Runtime-resolved config for a single AI channel.
#[derive(Debug, Clone)]
pub struct ResolvedAi {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Resolve the vision channel (env defaults unless a user choice overrides).
pub async fn resolve_vision(state: &AppState) -> ResolvedAi {
    let preset_id = get_setting(&state.db, SETTING_VISION_MODEL).await;
    resolve_channel(
        state,
        preset_id.as_deref(),
        ModelKind::Vision,
        &state.config.ai_base_url,
        &state.config.ai_api_key,
        &state.config.ai_model,
        state.config.ai_enabled,
    )
}

/// Resolve the chat channel.
pub async fn resolve_chat(state: &AppState) -> ResolvedAi {
    let preset_id = get_setting(&state.db, SETTING_CHAT_MODEL).await;
    resolve_channel(
        state,
        preset_id.as_deref(),
        ModelKind::Chat,
        &state.config.chat_base_url,
        &state.config.chat_api_key,
        &state.config.chat_model,
        state.config.chat_enabled,
    )
}

fn resolve_channel(
    state: &AppState,
    preset_id: Option<&str>,
    kind: ModelKind,
    default_url: &str,
    default_key: &str,
    default_model: &str,
    enabled: bool,
) -> ResolvedAi {
    // If the user chose a model, resolve its provider (preset first, then a
    // heuristic for models picked live from the provider's /models list).
    if let Some(id) = preset_id {
        let provider = presets()
            .into_iter()
            .find(|p| p.id == id && p.kind == kind)
            .map(|p| p.provider)
            .or_else(|| infer_provider(id));

        if let Some(provider) = provider {
            let (url, key) = match provider {
                Provider::Gemini => (
                    "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
                    state.config.ai_api_key.clone(),
                ),
                Provider::CommandCode => (
                    state.config.commandcode_base_url.clone(),
                    state.config.commandcode_api_key.clone(),
                ),
            };
            return ResolvedAi {
                enabled: enabled && !url.trim().is_empty(),
                base_url: url,
                api_key: key,
                model: id.to_string(),
            };
        }
    }

    // Otherwise fall back to env defaults.
    ResolvedAi {
        enabled: enabled && !default_url.trim().is_empty(),
        base_url: default_url.to_string(),
        api_key: default_key.to_string(),
        model: default_model.to_string(),
    }
}

/// Infer which provider a model id belongs to when it isn't a known preset.
/// Gemini ids look like `gemini-*` or `models/gemini-*`; anything else is
/// assumed to be a CommandCode (aggregator) id.
pub fn infer_provider(id: &str) -> Option<Provider> {
    let s = id.trim();
    if s.is_empty() {
        return None;
    }
    let bare = s.strip_prefix("models/").unwrap_or(s);
    if bare.starts_with("gemini-") || bare.starts_with("gemma-") {
        Some(Provider::Gemini)
    } else {
        Some(Provider::CommandCode)
    }
}

async fn get_setting(db: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
}

/// Persist a user's model choice.
pub async fn set_setting(db: &SqlitePool, key: &str, value: &str) -> Result<(), crate::error::AppError> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}

/// JSON representation for the settings page.
pub async fn config_json(state: &AppState) -> serde_json::Value {
    let vision = get_setting(&state.db, SETTING_VISION_MODEL).await;
    let chat = get_setting(&state.db, SETTING_CHAT_MODEL).await;

    serde_json::json!({
        "vision_model": vision,
        "chat_model": chat,
        "vision_presets": presets().into_iter()
            .filter(|p| p.kind == ModelKind::Vision)
            .map(|p| serde_json::json!({ "id": p.id, "label": p.label }))
            .collect::<Vec<_>>(),
        "chat_presets": presets().into_iter()
            .filter(|p| p.kind == ModelKind::Chat)
            .map(|p| serde_json::json!({ "id": p.id, "label": p.label }))
            .collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------------------
// Live model discovery: query each provider's /models endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetch the live model list from a provider's `/models` endpoint.
/// Returns an empty Vec on any failure (never errors out the request).
async fn fetch_models(base_url: &str, api_key: &str) -> Vec<String> {
    if base_url.trim().is_empty() {
        return Vec::new();
    }
    let endpoint = format!("{}/models", base_url.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut req = client.get(&endpoint);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }

    let resp = match req.send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            tracing::warn!(
                "models: {endpoint} returned {status}: {}",
                body.chars().take(300).collect::<String>()
            );
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!("models: {endpoint} transport error: {e}");
            return Vec::new();
        }
    };

    match resp.json::<ModelsResponse>().await {
        Ok(body) => {
            let mut ids: Vec<String> = body.data.into_iter().map(|m| m.id).collect();
            ids.sort();
            ids.dedup();
            tracing::info!("models: {} found via {endpoint}", ids.len());
            ids
        }
        Err(e) => {
            tracing::warn!("models: parse error from {endpoint}: {e}");
            Vec::new()
        }
    }
}

/// List available models from every configured provider.
pub async fn available_models(state: &AppState) -> serde_json::Value {
    let (gemini, commandcode) = tokio::join!(
        fetch_models(
            "https://generativelanguage.googleapis.com/v1beta/openai",
            &state.config.ai_api_key,
        ),
        fetch_models(
            &state.config.commandcode_base_url,
            &state.config.commandcode_api_key,
        ),
    );

    serde_json::json!({
        "gemini": gemini,
        "commandcode": commandcode,
        "gemini_configured": !state.config.ai_api_key.trim().is_empty(),
        "commandcode_configured": !state.config.commandcode_base_url.trim().is_empty(),
    })
}
