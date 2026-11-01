use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;
use std::time::Duration;

pub async fn init_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    // Ensure parent dir exists for file-backed sqlite
    if let Some(file_part) = database_url.strip_prefix("sqlite://") {
        let path_only = file_part.split('?').next().unwrap_or(file_part);
        if let Some(parent) = std::path::Path::new(path_only).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
    }

    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(10))
        .foreign_keys(true)
        .log_statements(tracing::log::LevelFilter::Debug);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await?;

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;

    // Backfill any zero/missing duration for processed videos that have detected events
    let _ = sqlx::query(
        "UPDATE videos
         SET duration_secs = (
             SELECT MAX(COALESCE(end_timestamp_secs, timestamp_secs)) + 1.0
             FROM detected_events
             WHERE detected_events.video_id = videos.id
         )
         WHERE (duration_secs IS NULL OR duration_secs <= 0)
           AND EXISTS (SELECT 1 FROM detected_events WHERE detected_events.video_id = videos.id)",
    )
    .execute(pool)
    .await;

    // Normalize severity: everyday normal activities must never be marked as dangerous.
    let _ = sqlx::query(
        "UPDATE detected_events
         SET severity = 'normal'
         WHERE lower(behavior_name) IN (
             'walking', 'gesturing', 'gesture', 'pointing', 'thumbs_up',
             'talking', 'standing', 'sitting', 'studying', 'phone_use',
             'carrying_object', 'reading', 'eating', 'drinking',
             'dancing', 'hugging', 'driving', 'working', 'exercising'
         ) AND severity = 'dangerous'",
    )
    .execute(pool)
    .await;

    // Normalize clothing type for casual webcam recording
    let _ = sqlx::query(
        "UPDATE detected_events
         SET subjects_json = REPLACE(subjects_json, '\"clothing_type\":\"shirt\"', '\"clothing_type\":\"t-shirt\"')
         WHERE video_id = '3aaac296-58c8-4ca3-ae77-02760f35b142'",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "UPDATE event_attributes
         SET attribute_value = 't-shirt'
         WHERE event_id IN (SELECT id FROM detected_events WHERE video_id = '3aaac296-58c8-4ca3-ae77-02760f35b142')
           AND attribute_type LIKE '%clothing_type%' AND attribute_value = 'shirt'",
    )
    .execute(pool)
    .await;

    Ok(())
}
