use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{ai_config, auth_api, chat, events, media, stats, videos};
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        .route("/health", get(health))
        // Auth
        .route("/auth/register", post(auth_api::register))
        .route("/auth/login", post(auth_api::login))
        .route("/auth/me", get(auth_api::me))
        // Videos
        .route(
            "/videos",
            post(videos::upload).layer(DefaultBodyLimit::max(1024 * 1024 * 1024)),
        )
        .route("/videos", get(videos::list))
        .route("/videos/import-library", post(videos::import_library))
        .route("/videos/:id", get(videos::get).delete(videos::delete))
        .route("/videos/:id/progress", get(videos::progress))
        .route("/videos/:id/process", post(videos::process))
        // Events
        .route("/events", get(events::list))
        .route("/events/:id", get(events::get))
        // Stats
        .route("/stats/overview", get(stats::overview))
        .route("/stats/videos/:id", get(stats::video_stats))
        // Chatbot
        .route("/chat/videos/:id", post(chat::ask))
        .route(
            "/chat/videos/:id/summary",
            get(chat::summary).post(chat::regenerate_summary),
        )
        // AI model selection
        .route("/ai/config", get(ai_config::get_config).post(ai_config::update_config))
        .route("/ai/models", get(ai_config::list_models))
        // Media (stream + frames)
        .route("/media/videos/:id/stream", get(media::stream_video))
        .route("/media/frames/:video_id/:index", get(media::serve_frame));

    let mut router = Router::new()
        .nest("/api", api)
        .layer(cors);

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".to_string());
    let static_path = std::path::Path::new(&static_dir);
    if static_path.exists() {
        let index_file = static_path.join("index.html");
        if index_file.exists() {
            let serve_dir = tower_http::services::ServeDir::new(static_path)
                .not_found_service(tower_http::services::ServeFile::new(index_file));
            router = router.fallback_service(serve_dir);
        }
    }

    router.with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "ok": true, "name": "IMWatching", "version": env!("CARGO_PKG_VERSION") }))
}
