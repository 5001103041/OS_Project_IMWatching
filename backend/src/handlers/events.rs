use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::models::DetectedEvent;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListParams {
    pub video_id: Option<String>,
    pub q: Option<String>,
    pub severity: Option<String>,
    pub behavior: Option<String>,
    pub gender: Option<String>,
}

/// GET /api/events — list/search/filter detected events.
/// Each event includes its `attributes` array (from event_attributes EAV).
pub async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<ListParams>,
) -> AppResult<Json<Value>> {
    // Build query scoped to the caller's videos.
    let mut sql = String::from(
        "SELECT e.* FROM detected_events e
         JOIN videos v ON v.id = e.video_id
         WHERE v.owner_id = ?",
    );
    let mut binds: Vec<String> = vec![auth.0.id.clone()];

    if let Some(vid) = &params.video_id {
        sql.push_str(" AND e.video_id = ?");
        binds.push(vid.clone());
    }
    if let Some(sev) = &params.severity {
        sql.push_str(" AND e.severity = ?");
        binds.push(sev.clone());
    }
    if let Some(b) = &params.behavior {
        sql.push_str(" AND e.behavior_name = ?");
        binds.push(b.clone());
    }
    if let Some(g) = &params.gender {
        sql.push_str(" AND e.gender = ?");
        binds.push(g.clone());
    }
    if let Some(q) = &params.q {
        let q = q.trim().to_lowercase();
        if !q.is_empty() {
            // Search behavior name, severity, gender, clothing_color, reason,
            // AND subjects JSON (clothing_type, glasses, hat, …) & event_attributes.
            sql.push_str(
                " AND (
                    lower(e.behavior_name) LIKE ?
                    OR lower(e.severity) LIKE ?
                    OR lower(e.gender) LIKE ?
                    OR lower(COALESCE(e.clothing_color,'')) LIKE ?
                    OR lower(COALESCE(e.ai_reason,'')) LIKE ?
                    OR lower(COALESCE(e.subjects_json,'')) LIKE ?
                    OR EXISTS (
                        SELECT 1 FROM event_attributes ea
                        WHERE ea.event_id = e.id
                          AND (lower(ea.attribute_type) LIKE ? OR lower(ea.attribute_value) LIKE ?)
                    )
                )",
            );
            let pattern = format!("%{}%", q);
            for _ in 0..8 {
                binds.push(pattern.clone());
            }
        }
    }

    sql.push_str(" ORDER BY e.timestamp_secs ASC LIMIT 500");

    let mut query = sqlx::query_as::<_, DetectedEvent>(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    let rows = query.fetch_all(&state.db).await?;

    // For the list view we fetch all attributes in one batch query and group by event_id.
    let event_ids: Vec<&str> = rows.iter().map(|e| e.id.as_str()).collect();
    let attrs_map = if event_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        fetch_attrs_batch(&state, &event_ids).await?
    };

    let events: Vec<Value> = rows
        .iter()
        .map(|e| event_json_with_attrs(e, attrs_map.get(&e.id).map(|v| v.as_slice())))
        .collect();

    Ok(Json(json!({ "events": events })))
}

/// GET /api/events/:id — single event detail with full attributes.
pub async fn get(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let event = sqlx::query_as::<_, DetectedEvent>(
        "SELECT e.* FROM detected_events e
         JOIN videos v ON v.id = e.video_id
         WHERE e.id = ? AND v.owner_id = ?",
    )
    .bind(&id)
    .bind(&auth.0.id)
    .fetch_optional(&state.db)
    .await?;

    let Some(event) = event else {
        return Ok(Json(json!({ "event": null })));
    };

    let attrs: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT attribute_type, attribute_value, confidence
         FROM event_attributes WHERE event_id = ? ORDER BY attribute_type",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;

    let mut obj = event_json_with_attrs(&event, Some(&attrs));
    // Also expose raw attribute list for backward compat.
    obj["attributes"] = json!(attrs
        .iter()
        .map(|(t, v, c)| json!({ "type": t, "value": v, "confidence": c }))
        .collect::<Vec<_>>());

    Ok(Json(obj))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

type AttrRow = (String, String, f64); // (attribute_type, attribute_value, confidence)

/// Batch-fetch attributes for a list of event IDs and group by event_id.
async fn fetch_attrs_batch(
    state: &AppState,
    event_ids: &[&str],
) -> Result<std::collections::HashMap<String, Vec<AttrRow>>, crate::error::AppError> {
    // SQLite doesn't support bind arrays — use IN (...) built dynamically.
    if event_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = event_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT event_id, attribute_type, attribute_value, confidence
         FROM event_attributes
         WHERE event_id IN ({placeholders})
         ORDER BY event_id, attribute_type"
    );
    let mut q = sqlx::query_as::<_, (String, String, String, f64)>(&sql);
    for id in event_ids {
        q = q.bind(*id);
    }
    let rows = q.fetch_all(&state.db).await?;

    let mut map: std::collections::HashMap<String, Vec<AttrRow>> = std::collections::HashMap::new();
    for (event_id, attr_type, attr_value, conf) in rows {
        map.entry(event_id).or_default().push((attr_type, attr_value, conf));
    }
    Ok(map)
}

/// Serialize a DetectedEvent + its attribute rows into a JSON Value,
/// including a rich `subjects` array with extended fields from `subjects_json`.
pub fn event_json_with_attrs(e: &DetectedEvent, attrs: Option<&[AttrRow]>) -> Value {
    // Deserialize subjects_json → full AiSubject structs (with extended fields).
    let subjects: Vec<Value> = e
        .subjects_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<crate::ai::AiSubject>>(s).ok())
        .map(|list| {
            list.into_iter()
                .map(|s| {
                    json!({
                        "gender": s.gender,
                        "clothing_color": s.clothing_color,
                        "clothing_type": s.clothing_type,
                        "glasses": s.glasses,
                        "hat": s.hat,
                        "bag": s.bag,
                        "age_group": s.age_group,
                        "mask": s.mask,
                        "hair_color": s.hair_color,
                        "confidence": s.confidence,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Build an attribute summary map: { "glasses": "yes", "hat": "no", … }
    // collapsed across all subjects for quick display.
    let mut attr_summary: serde_json::Map<String, Value> = attrs
        .unwrap_or(&[])
        .iter()
        .fold(serde_json::Map::new(), |mut acc, (typ, val, _conf)| {
            // Strip subject prefix (s0_, s1_, …) for the summary key.
            let key = typ
                .splitn(2, '_')
                .nth(1)
                .map(|s| s.to_string())
                .unwrap_or_else(|| typ.clone());
            // Only keep first occurrence per key (subject 0 is primary).
            acc.entry(key).or_insert_with(|| json!(val));
            acc
        });

    // Fallback: if EAV attrs is empty, derive attr_summary directly from subjects JSON.
    if attr_summary.is_empty() {
        if let Some(parsed_subjects) = e
            .subjects_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<crate::ai::AiSubject>>(s).ok())
        {
            for s in parsed_subjects {
                if s.glasses == Some(true) {
                    attr_summary.entry("glasses".to_string()).or_insert_with(|| json!("yes"));
                }
                if s.hat == Some(true) {
                    attr_summary.entry("hat".to_string()).or_insert_with(|| json!("yes"));
                }
                if s.mask == Some(true) {
                    attr_summary.entry("mask".to_string()).or_insert_with(|| json!("yes"));
                }
                if let Some(b) = &s.bag {
                    if b != "none" && b != "unknown" {
                        attr_summary.entry("bag".to_string()).or_insert_with(|| json!(b));
                    }
                }
                if let Some(ag) = &s.age_group {
                    if ag != "unknown" {
                        attr_summary.entry("age_group".to_string()).or_insert_with(|| json!(ag));
                    }
                }
                if let Some(ct) = &s.clothing_type {
                    if ct != "unknown" {
                        attr_summary.entry("clothing_type".to_string()).or_insert_with(|| json!(ct));
                    }
                }
                if let Some(hc) = &s.hair_color {
                    if hc != "unknown" {
                        attr_summary.entry("hair_color".to_string()).or_insert_with(|| json!(hc));
                    }
                }
            }
        }
    }

    json!({
        "id": e.id,
        "video_id": e.video_id,
        "timestamp_secs": e.timestamp_secs,
        "end_timestamp_secs": e.end_timestamp_secs,
        "behavior_name": e.behavior_name,
        "severity": e.severity,
        "gender": e.gender,
        "clothing_color": e.clothing_color,
        "confidence": e.confidence,
        "ai_reason": e.ai_reason,
        "subjects": subjects,
        "attr_summary": attr_summary,
        "frame_image_path": e.frame_image_path,
        "review_status": e.review_status,
        "created_at": e.created_at,
    })
}

/// Backward-compatible alias used by other modules (chatbot summary etc.).
pub fn event_json(e: &DetectedEvent) -> Value {
    event_json_with_attrs(e, None)
}
