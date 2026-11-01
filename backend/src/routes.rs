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
        .route("/media/frames/:video_id/:index", get(media::serve_frame))
        // OS Runtime & PCB Telemetry (Operating Systems course diagnostics)
        .route("/os/metrics", get(os_metrics))
        .route("/system/os", get(os_metrics))
        // Direct Hardware Telemetry (RAM, Disk, CPU, OS, Network)
        .route("/system/hardware", get(system_hardware))
        .route("/system/evaluate-upload", post(evaluate_upload));

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
    axum::Json(serde_json::json!({
        "ok": true,
        "name": "IMWatching",
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
    }))
}

async fn os_metrics(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::Json<crate::os_runtime::OsTelemetrySnapshot> {
    axum::Json(state.os.snapshot())
}

async fn system_hardware() -> axum::Json<crate::hardware_monitor::HardwareTelemetryReport> {
    let monitor = crate::hardware_monitor::global_hardware_monitor();
    axum::Json(monitor.get_telemetry())
}

#[derive(serde::Deserialize)]
struct EvaluateUploadPayload {
    file_size_bytes: Option<u64>,
    duration_secs: Option<f64>,
    resolution: Option<String>,
}

async fn evaluate_upload(
    axum::Json(payload): axum::Json<EvaluateUploadPayload>,
) -> axum::Json<crate::hardware_monitor::UploadHardwareReadiness> {
    let monitor = crate::hardware_monitor::global_hardware_monitor();
    let size = payload.file_size_bytes.unwrap_or(25_000_000);
    let eval = monitor.evaluate_video_upload(
        size,
        payload.duration_secs,
        payload.resolution.as_deref(),
    );
    axum::Json(eval)
}
