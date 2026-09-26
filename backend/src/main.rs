mod ai;
mod ai_config;
mod auth;
mod chatbot;
mod db;
mod error;
mod handlers;
mod jobs;
mod models;
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
    tracing::info!("IMWatching backend starting");

    let pool = db::init_pool(&config.database_url)
        .await
        .context("Initializing SQLite pool")?;

    db::run_migrations(&pool).await.context("Running migrations")?;

    // Ensure media dir exists.
    tokio::fs::create_dir_all(&config.media_dir).await.ok();

    let app_state = Arc::new(AppState::new(config.clone(), pool));

    let app = routes::build_router(app_state.clone()).layer(TraceLayer::new_for_http());

    let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
    tracing::info!("IMWatching backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
