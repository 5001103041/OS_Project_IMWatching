use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::state::AppState;

/// GET /api/stats/overview — focused security metrics:
/// - Total events & dangerous events ratio
/// - Total videos & dangerous videos ratio
/// - Breakdown of dangerous behaviors
/// - Per-video risk & danger breakdown (sorted by danger severity)
pub async fn overview(State(state): State<Arc<AppState>>, auth: AuthUser) -> AppResult<Json<Value>> {
    let total_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM detected_events e JOIN videos v ON v.id = e.video_id WHERE v.owner_id = ?")
            .bind(&auth.0.id)
            .fetch_one(&state.db)
            .await?;
    let dangerous_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM detected_events e JOIN videos v ON v.id = e.video_id WHERE v.owner_id = ? AND e.severity = 'dangerous'")
            .bind(&auth.0.id)
            .fetch_one(&state.db)
            .await?;
    let normal_events = total_events - dangerous_events;
    let dangerous_rate = if total_events > 0 {
        ((dangerous_events as f64 / total_events as f64) * 100.0).round() as i64
    } else {
        0
    };

    // Video counts & dangerous video ratio.
    let total_videos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM videos WHERE owner_id = ?")
            .bind(&auth.0.id)
            .fetch_one(&state.db)
            .await?;
    let dangerous_videos: i64 =
        sqlx::query_scalar(
            "SELECT COUNT(DISTINCT v.id) FROM videos v
             JOIN detected_events e ON e.video_id = v.id
             WHERE v.owner_id = ? AND e.severity = 'dangerous'",
        )
        .bind(&auth.0.id)
        .fetch_one(&state.db)
        .await?;
    let dangerous_video_rate = if total_videos > 0 {
        ((dangerous_videos as f64 / total_videos as f64) * 100.0).round() as i64
    } else {
        0
    };

    // Count strictly dangerous behaviors.
    let dangerous_behaviors: Vec<(String, i64)> = sqlx::query_as(
        "SELECT e.behavior_name, COUNT(*) FROM detected_events e
         JOIN videos v ON v.id = e.video_id
         WHERE v.owner_id = ? AND e.severity = 'dangerous'
         GROUP BY e.behavior_name
         ORDER BY COUNT(*) DESC",
    )
    .bind(&auth.0.id)
    .fetch_all(&state.db)
    .await?;

    // All behaviors with dangerous count per behavior.
    let by_behavior: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT e.behavior_name, COUNT(*),
                SUM(CASE WHEN e.severity = 'dangerous' THEN 1 ELSE 0 END)
         FROM detected_events e JOIN videos v ON v.id = e.video_id
         WHERE v.owner_id = ?
         GROUP BY e.behavior_name
         ORDER BY SUM(CASE WHEN e.severity = 'dangerous' THEN 1 ELSE 0 END) DESC, COUNT(*) DESC",
    )
    .bind(&auth.0.id)
    .fetch_all(&state.db)
    .await?;

    // Per-video breakdown with dangerous count & risk percentage.
    let by_video_rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT v.id, COALESCE(v.final_filename, v.original_filename) AS name,
                COUNT(e.id),
                SUM(CASE WHEN e.severity = 'dangerous' THEN 1 ELSE 0 END)
         FROM videos v LEFT JOIN detected_events e ON e.video_id = v.id
         WHERE v.owner_id = ?
         GROUP BY v.id, name
         ORDER BY SUM(CASE WHEN e.severity = 'dangerous' THEN 1 ELSE 0 END) DESC, COUNT(e.id) DESC",
    )
    .bind(&auth.0.id)
    .fetch_all(&state.db)
    .await?;

    let by_video: Vec<Value> = by_video_rows
        .into_iter()
        .map(|(id, name, total_ev, dang_ev)| {
            let rate = if total_ev > 0 {
                ((dang_ev as f64 / total_ev as f64) * 100.0).round() as i64
            } else {
                0
            };
            json!({
                "id": id,
                "name": name,
                "count": total_ev,
                "dangerous": dang_ev,
                "dangerous_rate": rate,
                "is_dangerous": dang_ev > 0,
            })
        })
        .collect();

    Ok(Json(json!({
        "total": total_events,
        "dangerous": dangerous_events,
        "normal": normal_events,
        "dangerous_rate": dangerous_rate,
        "total_videos": total_videos,
        "dangerous_videos": dangerous_videos,
        "dangerous_video_rate": dangerous_video_rate,
        "dangerous_behaviors": dangerous_behaviors.iter().map(|(b, c)| json!({"behavior": b, "count": c})).collect::<Vec<_>>(),
        "by_behavior": by_behavior.iter().map(|(b, total, dang)| json!({"behavior": b, "count": total, "dangerous": dang})).collect::<Vec<_>>(),
        "by_video": by_video,
    })))
}

/// GET /api/stats/videos/:id — aggregate for a single video.
pub async fn video_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?;
    if owner.as_deref() != Some(auth.0.id.as_str()) {
        return Ok(Json(json!({ "total": 0, "dangerous": 0, "normal": 0 })));
    }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM detected_events WHERE video_id = ?")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;
    let dangerous: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM detected_events WHERE video_id = ? AND severity = 'dangerous'")
            .bind(&id)
            .fetch_one(&state.db)
            .await?;

    let by_behavior: Vec<(String, i64)> = sqlx::query_as(
        "SELECT behavior_name, COUNT(*) FROM detected_events WHERE video_id = ? GROUP BY behavior_name ORDER BY COUNT(*) DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({
        "total": total,
        "dangerous": dangerous,
        "normal": total - dangerous,
        "by_behavior": by_behavior.iter().map(|(b, c)| json!({"behavior": b, "count": c})).collect::<Vec<_>>(),
    })))
}
