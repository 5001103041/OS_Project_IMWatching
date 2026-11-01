mod ai;
mod ai_config;
mod auth;
mod chatbot;
mod db;
mod error;
mod handlers;
mod hardware_monitor;
mod jobs;
mod models;
mod os_runtime;
mod routes;
mod state;
mod video;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn,tower_http=info".into()),
        )
        .init();

    let config = state::Config::from_env().context("Loading configuration from env")?;
    tracing::info!(
        "IMWatching backend starting (PID={}, OS={}, Arch={})",
        std::process::id(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let pool = db::init_pool(&config.database_url)
        .await
        .context("Initializing SQLite pool")?;

    db::run_migrations(&pool).await.context("Running migrations")?;

    // Ensure media dir exists.
    tokio::fs::create_dir_all(&config.media_dir).await.ok();

    let app_state = Arc::new(AppState::new(config.clone(), pool));
    let os_snap = app_state.os.snapshot();
    tracing::info!(
        "OS Worker Pool initialized: max_pipelines={}, max_subprocesses={}, cpu_cores={}",
        os_snap.synchronization.max_pipeline_permits,
        os_snap.synchronization.max_subprocess_permits,
        os_snap.cpu_cores
    );

    let app = routes::build_router(app_state.clone()).layer(TraceLayer::new_for_http());

    let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
    tracing::info!("IMWatching backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(app_state.clone()))
        .await?;

    // Flush SQLite WAL and release database file descriptors cleanly before process exit.
    tracing::info!("Closing SQLite pool and releasing OS file descriptors...");
    app_state.db.close().await;
    tracing::info!("IMWatching backend exited cleanly (PID={})", std::process::id());

    Ok(())
}

/// Bắt tín hiệu ngắt từ Hệ điều hành (`SIGINT` / Ctrl+C hoặc `SIGTERM` trên Unix)
/// để thông báo cho các luồng Worker dừng nhận việc mới và giải phóng tài nguyên an toàn.
async fn shutdown_signal(state: Arc<AppState>) {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C (SIGINT) handler: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received OS signal SIGINT (Ctrl+C) — initiating graceful shutdown");
        },
        _ = terminate => {
            tracing::info!("Received OS signal SIGTERM — initiating graceful shutdown");
        },
    }

    state.os.request_shutdown();
}
