//! Chatbot: RAG over analyzed events of a video, powered by a cached summary.
//!
//! Flow:
//!   1. `get_or_generate_summary()` — reads `videos.ai_summary` from DB.
//!      Cache hit  → return immediately (no AI call).
//!      Cache miss → call `summarize()` → write back to DB → return.
//!   2. `answer()` — uses the cached summary as context instead of dumping
//!      all raw events, saving tokens and latency on every question.
//!   3. `summarize()` — generates the holistic 3-part summary (offline or AI).
//!
//! Offline fallback: when AI is disabled the deterministic `offline_summary()`
//! is used everywhere; the result is still cached in the DB.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::ai::AiSubject;
use crate::error::AppError;
use crate::models::DetectedEvent;
use crate::state::AppState;

// ─── Data types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventContext {
    pub id: String,
    pub timestamp_secs: f64,
    pub end_timestamp_secs: Option<f64>,
    pub behavior_name: String,
    pub severity: String,
    pub gender: String,
    pub clothing_color: Option<String>,
    pub confidence: f64,
    pub ai_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Vec<AiSubject>>,
}

// ─── System prompts ──────────────────────────────────────────────────────────

/// System prompt for the QA chatbot — context is now the pre-built summary,
/// not the raw event array, so it is short and token-efficient.
const QA_SYSTEM_PROMPT: &str = "\
Bạn là trợ lý phân tích video thông minh. \
Bạn được cung cấp bản tổng hợp toàn cảnh và danh sách đặc điểm đối tượng \
của một video đã qua phân tích AI.

Quy tắc:
- Trả lời dựa trên TOÀN BỘ thông tin được cung cấp (cả tổng hợp lẫn danh sách đặc điểm).
- Nếu câu hỏi hỏi về đặc điểm cụ thể (đeo kính, đội mũ, màu áo, giới tính, phụ kiện...), \
  hãy tra cứu kỹ trong phần \"Đặc điểm chi tiết\" trước khi trả lời.
- Trả lời bằng tiếng Việt tự nhiên, ngắn gọn và chính xác.
- Khi hỏi về thời điểm, trích dẫn mốc thời gian (mm:ss) nếu có.
- Tuyệt đối KHÔNG dùng các ký tự markdown dư thừa như *** hoặc ** hoặc gạch chân. Hãy trình bày văn bản sạch sẽ, rõ ràng.
- Chỉ nói \"Không đủ dữ liệu\" khi thực sự không tìm thấy bất kỳ thông tin liên quan nào.
";

/// System prompt for generating the intelligent 3-part summary.
const SUMMARIZE_SYSTEM_PROMPT: &str = "\
Bạn là chuyên gia an ninh và phân tích video thông minh (Video Intelligence Analyst).
Nhiệm vụ của bạn là đọc toàn bộ nhật ký sự kiện trích xuất từ camera và viết một bản báo cáo tóm tắt thông minh, mạch lạc, chính xác bằng tiếng Việt.

Bản tóm tắt PHẢI tuân theo đúng cấu trúc 3 phần sau:

1. Sự kiện & Ngữ cảnh: <Thời lượng video (ví dụ: Video dài 32s), ghi nhận tổng số sự kiện và số sự kiện nguy hiểm/cảnh báo, bối cảnh không gian/môi trường nếu có>.
2. Hành động chi tiết: <Tóm tắt thông minh và mạch lạc diễn biến hành động theo các khoảng thời gian [mm:ss - mm:ss], kết hợp rõ số lượng đối tượng và giới tính/trang phục tham gia (ví dụ: 'Khoảng 6 đối tượng nam đang tham gia đánh nhau, xô xát liên tục trên đường/hiện trường từ [00:00] đến [00:31], sau đó là hành vi đi bộ rời đi lúc [00:31 - 00:32]'). TUYỆT ĐỐI KHÔNG liệt kê danh sách rời rạc từng giây lặp đi lặp lại như 'fighting (00:00), fighting (00:01)...'>.
3. Ước lượng đối tượng: <Ước lượng số lượng đối tượng riêng biệt xuất hiện trong video, đặc điểm nhận diện chi tiết: giới tính, màu áo, loại áo, các phụ kiện (đeo kính, đội mũ, đeo khẩu trang, mang ba lô/túi xách), màu tóc nếu có>.

Nguyên tắc:
- Tiếng Việt chuẩn xác, hành văn tự nhiên, súc tích và giàu thông tin.
- Dùng tên hành vi và cử chỉ hoàn toàn bằng tiếng Việt (ví dụ: đánh nhau, đi bộ, chạy, ngã, giơ ngón tay cái, tiếp xúc va chạm...), tuyệt đối không dùng tiếng Anh.
- Tuyệt đối không dùng các dấu hoa thị thô như *** hoặc ** xung quanh tiêu đề hay đầu dòng. Hãy viết văn bản sạch sẽ, tự nhiên, dễ đọc.
- Căn cứ 100% trên dữ liệu sự kiện đã trích xuất, không bịa đặt chi tiết không có.";

// ─── Localization helpers ────────────────────────────────────────────────────

pub fn behavior_to_vn(b: &str) -> &'static str {
    match b.trim().to_lowercase().as_str() {
        "fighting" => "đánh nhau",
        "aggression" => "gây hấn",
        "aggressive" => "hung hăng",
        "aggressive_behavior" => "hành vi hung hăng",
        "aggressive_expression" => "biểu cảm hung hăng",
        "angry_expression" => "biểu cảm giận dữ",
        "aggressive_posing" => "tư thế hung hăng",
        "angry" => "giận dữ",
        "violence" => "bạo lực",
        "school_violence" => "bạo lực học đường",
        "assault" => "tấn công hành hung",
        "threat" | "threatening" => "đe dọa",
        "weapon" | "weapon_use" | "weapon_holding" => "sử dụng vũ khí",
        "knife" => "dao",
        "gun" => "súng",
        "stick" => "gậy",
        "punching" => "đấm",
        "kicking" => "đá",
        "slapping" => "tát",
        "choking" | "strangling" => "bóp cổ",
        "grappling" | "wrestling" => "vật lộn",
        "taunting" | "provoking" => "khiêu khích",
        "argument" | "quarreling" => "cãi vã",
        "pushing" | "shoving" => "xô đẩy",
        "vandalism" => "phá hoại",
        "theft" | "stealing" => "trộm cắp",
        "harassment" => "quấy rối",
        "walking" => "đi bộ",
        "standing" => "đứng",
        "sitting" => "ngồi",
        "running" => "chạy",
        "falling" => "ngã",
        "talking" => "nói chuyện",
        "studying" => "ngồi học",
        "driving" => "lái xe",
        "no_hands_driving" => "lái xe bỏ 2 tay",
        "exercising" => "tập thể dục",
        "working" => "làm việc",
        "pointing" => "chỉ tay",
        "carrying_object" => "mang vác vật",
        "phone_use" => "dùng điện thoại",
        "smoking" => "hút thuốc",
        "hugging" => "ôm",
        "dancing" => "nhảy múa",
        "gesturing" => "ra hiệu",
        "gesture" => "cử chỉ",
        "thumbs_up" => "giơ ngón tay cái",
        "physical_contact" => "tiếp xúc va chạm",
        "verbal" => "giao tiếp bằng lời",
        "proximity" => "ở cự ly gần",
        "unanalyzed" => "chưa phân tích",
        _ => "hành vi khác",
    }
}

pub fn gender_to_vn(g: &str) -> &'static str {
    match g.trim().to_lowercase().as_str() {
        "male" => "nam",
        "female" => "nữ",
        _ => "không rõ giới tính",
    }
}

pub fn color_to_vn(c: &str) -> &'static str {
    match c.trim().to_lowercase().as_str() {
        "red" => "đỏ",
        "white" => "trắng",
        "black" => "đen",
        "blue" => "xanh dương",
        "green" => "xanh lá",
        "yellow" => "vàng",
        "orange" => "cam",
        "pink" => "hồng",
        "purple" => "tím",
        "brown" => "nâu",
        "gray" | "grey" => "xám",
        "blonde" | "golden" => "vàng",
        "dark" => "tối màu",
        "light" => "sáng màu",
        "unknown" | "" => "không rõ màu",
        _ => "không rõ màu",
    }
}

pub fn clothing_type_to_vn<'a>(t: &'a str) -> &'a str {
    match t.trim() {
        s if s.eq_ignore_ascii_case("shirt") => "áo sơ mi",
        s if s.eq_ignore_ascii_case("t-shirt") => "áo thun",
        s if s.eq_ignore_ascii_case("hoodie") => "áo hoodie",
        s if s.eq_ignore_ascii_case("jacket") => "áo khoác",
        s if s.eq_ignore_ascii_case("dress") => "váy",
        s if s.eq_ignore_ascii_case("uniform") => "đồng phục",
        s if s.eq_ignore_ascii_case("sweater") => "áo len",
        s if s.eq_ignore_ascii_case("tank-top") => "áo ba lỗ",
        _ => t,
    }
}

// ─── Shared HTTP helpers ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatRespMessage,
}

#[derive(Deserialize)]
struct ChatRespMessage {
    content: String,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Fetch raw events for a video (ownership-checked).
pub async fn fetch_context(
    state: &AppState,
    owner_id: &str,
    video_id: &str,
) -> Result<Vec<EventContext>, AppError> {
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM videos WHERE id = ?")
        .bind(video_id)
        .fetch_optional(&state.db)
        .await?;
    if owner.as_deref() != Some(owner_id) {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query_as::<_, DetectedEvent>(
        "SELECT * FROM detected_events WHERE video_id = ? ORDER BY timestamp_secs ASC",
    )
    .bind(video_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .iter()
        .map(|e| {
            let subjects: Option<Vec<AiSubject>> = e
                .subjects_json
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok());

            EventContext {
                id: e.id.clone(),
                timestamp_secs: e.timestamp_secs,
                end_timestamp_secs: e.end_timestamp_secs,
                behavior_name: e.behavior_name.clone(),
                severity: e.severity.clone(),
                gender: e.gender.clone(),
                clothing_color: e.clothing_color.clone(),
                confidence: e.confidence,
                ai_reason: e.ai_reason.clone(),
                subjects,
            }
        })
        .collect())
}

/// Return the cached summary for `video_id`, generating and persisting it on
/// the first call (lazy, cache-aside pattern).
///
/// Ownership is verified inside `fetch_context`.
pub async fn get_or_generate_summary(
    state: &AppState,
    owner_id: &str,
    video_id: &str,
) -> Result<String, AppError> {
    // 1. Check cache ─────────────────────────────────────────────────────────
    let cached: Option<String> =
        sqlx::query_scalar("SELECT ai_summary FROM videos WHERE id = ?")
            .bind(video_id)
            .fetch_optional(&state.db)
            .await?
            .flatten();

    if let Some(summary) = cached {
        if !summary.trim().is_empty() {
            tracing::debug!("summary cache hit for video {video_id}");
            return Ok(summary);
        }
    }

    // 2. Cache miss — generate ───────────────────────────────────────────────
    tracing::info!("summary cache miss for video {video_id} — generating");
    let summary = summarize(state, owner_id, video_id).await?;

    // 3. Persist ─────────────────────────────────────────────────────────────
    sqlx::query(
        "UPDATE videos \
         SET ai_summary = ?, ai_summary_generated_at = datetime('now'), updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(&summary)
    .bind(video_id)
    .execute(&state.db)
    .await?;

    tracing::info!("summary cached for video {video_id}");
    Ok(summary)
}

/// Answer a user question about a video using the cached summary as context.
///
/// Token cost per question: O(summary_length) instead of O(all_events_length).
pub async fn answer(
    state: &AppState,
    owner_id: &str,
    video_id: &str,
    question: &str,
) -> Result<String, AppError> {
    // Get (or lazily generate) the summary — this also checks ownership.
    let summary = get_or_generate_summary(state, owner_id, video_id).await?;

    // Fallback if chat AI is disabled.
    let cfg = crate::ai_config::resolve_chat(state).await;
    if !cfg.enabled || cfg.base_url.trim().is_empty() {
        return Ok(fallback_answer_from_summary(&summary, question));
    }

    // Pull structured attribute data from event_attributes table to supplement
    // the summary. This ensures specific queries (glasses, hat, bag, etc.) are
    // answered accurately even if the AI summary omitted them.
    let attr_lines = fetch_attribute_summary(state, video_id).await;
    let attr_section = if attr_lines.is_empty() {
        String::new()
    } else {
        format!("\n\nĐặc điểm chi tiết (từ phân tích frame):\n{}", attr_lines.join("\n"))
    };

    let user_msg = format!(
        "Tổng hợp video:\n{summary}{attr_section}\n\nCâu hỏi: {question}\n\nTrả lời:"
    );

    let request = ChatRequest {
        model: cfg.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system",
                content: QA_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user",
                content: user_msg,
            },
        ],
        max_tokens: Some(400),
        temperature: Some(0.0),
    };

    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(state.config.ai_timeout_secs.max(5)))
        .build()
        .map_err(|e| AppError::Internal(format!("client: {e}")))?;

    const MAX_ATTEMPTS: u32 = 3;
    let mut delay_ms: u64 = 1000;
    for attempt in 1..=MAX_ATTEMPTS {
        let mut req = client.post(&endpoint).json(&request);
        if !cfg.api_key.trim().is_empty() {
            req = req.bearer_auth(&cfg.api_key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: ChatResponse = match resp.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("chat: parse error: {e}");
                        return Ok(fallback_answer_from_summary(&summary, question));
                    }
                };
                let answer = body
                    .choices
                    .into_iter()
                    .next()
                    .map(|c| c.message.content)
                    .unwrap_or_else(|| fallback_answer_from_summary(&summary, question));
                return Ok(answer.trim().to_string());
            }
            Ok(resp) => {
                let status = resp.status();
                let retryable = status.as_u16() == 429 || status.is_server_error();
                if !retryable || attempt == MAX_ATTEMPTS {
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!("chat: non-success {status}, body: {body}");
                    return Ok(fallback_answer_from_summary(&summary, question));
                }
                tracing::warn!("chat: attempt {attempt}/{MAX_ATTEMPTS} got {status}, retrying");
            }
            Err(e) => {
                if attempt == MAX_ATTEMPTS {
                    tracing::warn!("chat: giving up: {e}");
                    return Ok(fallback_answer_from_summary(&summary, question));
                }
                tracing::warn!("chat: attempt {attempt}/{MAX_ATTEMPTS} error: {e}");
            }
        }

        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        delay_ms *= 2;
    }

    Ok(fallback_answer_from_summary(&summary, question))
}

/// Generate the holistic 3-part summary from raw events.
///
/// Called by `get_or_generate_summary()` on cache miss.
/// Also exposed via `GET /api/chat/videos/:id/summary`.
pub async fn summarize(
    state: &AppState,
    owner_id: &str,
    video_id: &str,
) -> Result<String, AppError> {
    let ctx = fetch_context(state, owner_id, video_id).await?;
    if ctx.is_empty() {
        return Ok("Chưa có dữ liệu phân tích cho video này.".to_string());
    }

    // Offline deterministic summary (no AI needed).
    let cfg = crate::ai_config::resolve_chat(state).await;
    if !cfg.enabled || cfg.base_url.trim().is_empty() {
        return Ok(offline_summary(&ctx));
    }

    // Video metadata
    let video_row: Option<(Option<String>, Option<f64>)> = sqlx::query_as(
        "SELECT original_filename, duration_secs FROM videos WHERE id = ?"
    )
    .bind(video_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or_default();

    let duration_info = video_row
        .and_then(|r| r.1)
        .map(|d| format!("Độ dài video: {:.1} giây ({:02}:{:02}).", d, (d / 60.0) as u32, (d % 60.0) as u32))
        .unwrap_or_default();

    // Format events chronologically as clean, readable bullet points (instead of noisy raw JSON)
    let mut timeline_entries: Vec<String> = Vec::new();
    for e in &ctx {
        let start_m = (e.timestamp_secs / 60.0) as u32;
        let start_s = (e.timestamp_secs % 60.0) as u32;
        let end_secs = e.end_timestamp_secs.unwrap_or(e.timestamp_secs);
        let end_m = (end_secs / 60.0) as u32;
        let end_s = (end_secs % 60.0) as u32;

        let b_vn = behavior_to_vn(&e.behavior_name);
        let subjs_desc = if let Some(subjs) = &e.subjects {
            subjs.iter().map(|s| {
                let mut parts = Vec::new();
                if !s.gender.is_empty() { parts.push(gender_to_vn(&s.gender).to_string()); }
                if !s.clothing_color.is_empty() { parts.push(format!("áo {}", color_to_vn(&s.clothing_color))); }
                if let Some(ct) = &s.clothing_type { if ct != "unknown" { parts.push(clothing_type_to_vn(ct).to_string()); } }
                if s.glasses == Some(true) { parts.push("đeo kính".into()); }
                if s.hat == Some(true) { parts.push("đội mũ".into()); }
                if s.mask == Some(true) { parts.push("đeo khẩu trang".into()); }
                if let Some(b) = &s.bag {
                    if b != "unknown" && b != "none" {
                        let bag_vn = match b.as_str() { "backpack" => "ba lô", "handbag" => "túi xách", _ => b };
                        parts.push(format!("túi/ba lô: {bag_vn}"));
                    }
                }
                if let Some(hc) = &s.hair_color {
                    if hc != "unknown" { parts.push(format!("tóc {}", color_to_vn(hc))); }
                }
                parts.join(", ")
            }).collect::<Vec<_>>().join(" | ")
        } else {
            format!("{} áo {}", gender_to_vn(&e.gender), color_to_vn(e.clothing_color.as_deref().unwrap_or("unknown")))
        };

        let sev_tag = if e.severity == "dangerous" { " [CẢNH BÁO NGUY HIỂM]" } else { "" };
        let reason_part = if let Some(r) = &e.ai_reason {
            if !r.trim().is_empty() { format!(" - Lý do: {}", r.trim()) } else { String::new() }
        } else {
            String::new()
        };

        timeline_entries.push(format!(
            "- [{:02}:{:02} - {:02}:{:02}] Hành vi: {}{}{} | Đối tượng: ({})",
            start_m, start_s, end_m, end_s, b_vn, sev_tag, reason_part, subjs_desc
        ));
    }

    // Granular subject attributes
    let attr_lines = fetch_attribute_summary(state, video_id).await;
    let attr_section = if attr_lines.is_empty() {
        String::new()
    } else {
        format!("\n\nĐặc điểm nhận diện đối tượng đã ghi nhận:\n{}", attr_lines.join("\n"))
    };

    // Fetch scene data for richer context.
    let scene_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT scene_json FROM video_frames WHERE video_id = ? AND scene_json IS NOT NULL LIMIT 5"
    )
    .bind(video_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let scene_context = if !scene_rows.is_empty() {
        let scenes: Vec<&str> = scene_rows.iter().map(|r| r.0.as_str()).collect();
        format!("\n\nBối cảnh không gian mẫu trích xuất:\n{}", scenes.join("\n"))
    } else {
        String::new()
    };

    let user_msg = format!(
        "DỮ LIỆU PHÂN TÍCH TỔNG QUAN:\n{}\nTổng số sự kiện trích xuất: {}\n\nNHẬT KÝ SỰ KIỆN THEO THỜI GIAN:\n{}{}{}\n\nHãy phân tích và tổng hợp toàn cảnh thành bản báo cáo thông minh theo đúng 3 phần yêu cầu.",
        duration_info,
        ctx.len(),
        timeline_entries.join("\n"),
        attr_section,
        scene_context
    );

    match chat_once(state, SUMMARIZE_SYSTEM_PROMPT, &user_msg, 1200).await {
        Ok(s) => Ok(s),
        Err(_) => Ok(offline_summary(&ctx)), // degrade gracefully
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Generic single-turn chat call using the chat channel config.
async fn chat_once(
    state: &AppState,
    system: &str,
    user: &str,
    max_tokens: u32,
) -> Result<String, AppError> {
    let cfg = crate::ai_config::resolve_chat(state).await;
    if !cfg.enabled || cfg.base_url.trim().is_empty() {
        return Err(AppError::Internal("chat disabled".into()));
    }

    let request = ChatRequest {
        model: cfg.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system",
                content: system.to_string(),
            },
            ChatMessage {
                role: "user",
                content: user.to_string(),
            },
        ],
        max_tokens: Some(max_tokens),
        temperature: Some(0.2),
    };

    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(state.config.ai_timeout_secs.max(5)))
        .build()
        .map_err(|e| AppError::Internal(format!("client: {e}")))?;

    const MAX_ATTEMPTS: u32 = 3;
    let mut delay_ms: u64 = 1000;
    for attempt in 1..=MAX_ATTEMPTS {
        let mut req = client.post(&endpoint).json(&request);
        if !cfg.api_key.trim().is_empty() {
            req = req.bearer_auth(&cfg.api_key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: ChatResponse = match resp.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("summary: parse error: {e}");
                        return Err(AppError::Internal("parse error".into()));
                    }
                };
                if let Some(c) = body.choices.into_iter().next() {
                    return Ok(c.message.content.trim().to_string());
                }
                return Err(AppError::Internal("empty response".into()));
            }
            Ok(resp) => {
                let status = resp.status();
                let retryable = status.as_u16() == 429 || status.is_server_error();
                if !retryable || attempt == MAX_ATTEMPTS {
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!("summary: non-success {status}, body: {body}");
                    return Err(AppError::Internal(format!("status {status}")));
                }
                tracing::warn!("summary: attempt {attempt}/{MAX_ATTEMPTS} got {status}, retrying");
            }
            Err(e) => {
                if attempt == MAX_ATTEMPTS {
                    tracing::warn!("summary: giving up: {e}");
                    return Err(AppError::Internal(e.to_string()));
                }
                tracing::warn!("summary: attempt {attempt}/{MAX_ATTEMPTS} error: {e}");
            }
        }
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        delay_ms *= 2;
    }

    Err(AppError::Internal("summary failed".into()))
}

/// Deterministic offline summary — 3-part structure matching user expectations, no AI required.
fn offline_summary(ctx: &[EventContext]) -> String {
    let total = ctx.len();
    let dangerous = ctx.iter().filter(|e| e.severity == "dangerous").count();

    // Distinct behaviors in chronological order of first appearance.
    let mut seen = std::collections::HashSet::new();
    let mut behaviors: Vec<String> = Vec::new();
    for e in ctx {
        if seen.insert(e.behavior_name.clone()) {
            behaviors.push(e.behavior_name.clone());
        }
    }

    // Merge consecutive events with the same behavior into continuous intervals
    struct ActionInterval {
        behavior: String,
        severity: String,
        start_secs: f64,
        end_secs: f64,
        subject_count: usize,
        genders: Vec<String>,
        colors: Vec<String>,
    }

    let mut intervals: Vec<ActionInterval> = Vec::new();
    let mut max_subjects_overall = 0;
    let mut person_features = std::collections::HashSet::new();

    for e in ctx {
        let e_start = e.timestamp_secs;
        let e_end = e.end_timestamp_secs.unwrap_or(e.timestamp_secs + 1.0);
        let subjs = e.subjects.as_deref().unwrap_or(&[]);
        let subj_count = if subjs.is_empty() { 1 } else { subjs.len() };
        max_subjects_overall = max_subjects_overall.max(subj_count);

        let mut g_list = Vec::new();
        let mut c_list = Vec::new();

        if let Some(subjs) = &e.subjects {
            for s in subjs {
                if !s.gender.is_empty() {
                    g_list.push(s.gender.clone());
                }
                if !s.clothing_color.is_empty() {
                    c_list.push(s.clothing_color.clone());
                }
                let mut feat = Vec::new();
                if !s.gender.is_empty() {
                    let g_vn = gender_to_vn(&s.gender);
                    if g_vn != "không rõ giới tính" { feat.push(g_vn.to_string()); }
                }
                if !s.clothing_color.is_empty() {
                    feat.push(format!("áo {}", color_to_vn(&s.clothing_color)));
                }
                if let Some(ct) = &s.clothing_type {
                    if ct != "unknown" { feat.push(clothing_type_to_vn(ct).to_string()); }
                }
                if s.glasses == Some(true) { feat.push("đeo kính".to_string()); }
                if s.hat == Some(true) { feat.push("đội mũ".to_string()); }
                if s.mask == Some(true) { feat.push("đeo khẩu trang".to_string()); }
                if !feat.is_empty() { person_features.insert(feat.join(", ")); }
            }
        } else {
            g_list.push(e.gender.clone());
            let key = format!("{} áo {}", gender_to_vn(&e.gender), color_to_vn(e.clothing_color.as_deref().unwrap_or("không rõ")));
            person_features.insert(key);
        }

        if let Some(last) = intervals.last_mut() {
            // If same behavior and within 2.5s gap, extend interval
            if last.behavior == e.behavior_name && (e_start - last.end_secs).abs() <= 2.5 {
                last.end_secs = last.end_secs.max(e_end);
                last.subject_count = last.subject_count.max(subj_count);
                for g in g_list { if !last.genders.contains(&g) { last.genders.push(g); } }
                for c in c_list { if !last.colors.contains(&c) { last.colors.push(c); } }
                continue;
            }
        }

        intervals.push(ActionInterval {
            behavior: e.behavior_name.clone(),
            severity: e.severity.clone(),
            start_secs: e_start,
            end_secs: e_end,
            subject_count: subj_count,
            genders: g_list,
            colors: c_list,
        });
    }

    let first_ts = ctx.first().map(|e| e.timestamp_secs).unwrap_or(0.0);
    let last_ts = ctx
        .last()
        .map(|e| e.end_timestamp_secs.unwrap_or(e.timestamp_secs))
        .unwrap_or(0.0);
    let duration = (last_ts - first_ts).max(1.0);

    // Build intelligent Section 2 narrative
    let mut interval_lines: Vec<String> = Vec::new();
    for iv in &intervals {
        let sm = (iv.start_secs / 60.0) as u32;
        let ss = (iv.start_secs % 60.0) as u32;
        let em = (iv.end_secs / 60.0) as u32;
        let es = (iv.end_secs % 60.0) as u32;

        let b_vn = match iv.behavior.as_str() {
            "fighting" => "đang tham gia đánh nhau, xô xát",
            "aggression" => "có hành vi gây hấn",
            "aggressive" => "có hành vi hung hăng",
            "violence" => "có hành vi bạo lực",
            "school_violence" => "đang xảy ra bạo lực học đường",
            "assault" => "tấn công hành hung",
            "walking" => "đi bộ",
            "standing" => "đứng",
            "sitting" => "ngồi",
            "running" => "chạy",
            "falling" => "ngã",
            "talking" => "nói chuyện",
            "studying" => "ngồi học",
            "no_hands_driving" => "lái xe bỏ 2 tay",
            "carrying_object" => "mang vác vật nặng",
            "phone_use" => "sử dụng điện thoại",
            "smoking" => "hút thuốc",
            "hugging" => "ôm",
            "dancing" => "nhảy múa",
            "gesturing" => "ra hiệu",
            "thumbs_up" => "giơ ngón tay cái",
            "physical_contact" => "tiếp xúc va chạm",
            "verbal" => "giao tiếp bằng lời",
            "proximity" => "ở cự ly gần",
            _ => behavior_to_vn(&iv.behavior),
        };

        let gender_vn = if iv.genders.iter().any(|g| g == "male") && iv.genders.iter().any(|g| g == "female") {
            "cả nam và nữ"
        } else if iv.genders.iter().any(|g| g == "male") {
            "nam"
        } else if iv.genders.iter().any(|g| g == "female") {
            "nữ"
        } else {
            "đối tượng"
        };

        let subj_text = if iv.subject_count > 1 {
            format!("khoảng {} đối tượng {}", iv.subject_count, gender_vn)
        } else {
            format!("1 đối tượng {}", gender_vn)
        };

        let warn_tag = if iv.severity == "dangerous" { " [⚠ NGUY HIỂM]" } else { "" };

        interval_lines.push(format!(
            "- Từ [{:02}:{:02}] đến [{:02}:{:02}]: Ghi nhận {} {}{}.",
            sm, ss, em, es, subj_text, b_vn, warn_tag
        ));
    }

    let action_narrative = if interval_lines.is_empty() {
        behaviors.iter().map(|b| behavior_to_vn(b)).collect::<Vec<_>>().join(" → ")
    } else {
        interval_lines.join("\n")
    };

    let features_str = if person_features.is_empty() {
        format!("khoảng {} đối tượng riêng biệt được nhận diện.", max_subjects_overall.max(1))
    } else {
        let list: Vec<String> = person_features.into_iter().take(6).collect();
        format!(
            "khoảng {} đối tượng riêng biệt được nhận diện. Đặc điểm ghi nhận: {}.",
            max_subjects_overall.max(1),
            list.join("; ")
        )
    };

    format!(
        "1. Sự kiện & Ngữ cảnh: Video dài {:.0}s, ghi nhận {total} sự kiện ({dangerous} nguy hiểm).\n\
         2. Hành động chi tiết:\n{}\n\
         3. Ước lượng đối tượng: {}",
        duration,
        action_narrative,
        features_str,
    )
}

/// Fetch a compact, human-readable list of subject attributes for a video
/// directly from the `event_attributes` table. Used to supplement the AI
/// summary with granular data that the summary prose may have omitted.
///
/// Returns lines like:
///   - "Người mặc áo nâu (nam): đeo kính"
///   - "Người mặc áo trắng (nữ): đội mũ, ba lô"
async fn fetch_attribute_summary(state: &AppState, video_id: &str) -> Vec<String> {
    // Get all events with their subjects_json + key attribute values from
    // the EAV table in a single query.
    let rows: Vec<(String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT e.gender, e.clothing_color, ea.attribute_type, ea.attribute_value
         FROM detected_events e
         JOIN event_attributes ea ON ea.event_id = e.id
         WHERE e.video_id = ?
           AND ea.attribute_type IN (
               's0_glasses','s0_hat','s0_mask','s0_bag','s0_hair_color','s0_age_group','s0_clothing_type',
               's1_glasses','s1_hat','s1_mask','s1_bag','s1_hair_color',
               'interaction_type'
           )
         ORDER BY e.timestamp_secs ASC"
    )
    .bind(video_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        // Fallback: parse subjects_json directly for glasses/hat/mask.
        let subj_rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT subjects_json FROM detected_events
             WHERE video_id = ? AND subjects_json IS NOT NULL LIMIT 50"
        )
        .bind(video_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let mut lines: Vec<String> = Vec::new();
        for (json_str,) in subj_rows {
            if let Some(s) = json_str {
                if let Ok(subjects) = serde_json::from_str::<Vec<crate::ai::AiSubject>>(&s) {
                    for subj in subjects {
                        let mut feats: Vec<String> = Vec::new();
                        if subj.glasses == Some(true) { feats.push("đeo kính".to_string()); }
                        if subj.hat == Some(true) { feats.push("đội mũ".to_string()); }
                        if subj.mask == Some(true) { feats.push("đeo khẩu trang".to_string()); }
                        if let Some(hc) = &subj.hair_color {
                            if hc != "unknown" { feats.push(format!("tóc {hc}")); }
                        }
                        if !feats.is_empty() {
                            let gender = gender_to_vn(&subj.gender);
                            let color = color_to_vn(&subj.clothing_color);
                            lines.push(format!(
                                "- Người {} áo {}: {}",
                                gender,
                                color,
                                feats.join(", ")
                            ));
                        }
                    }
                }
            }
        }
        lines.dedup();
        return lines;
    }

    // Group attributes by (gender, clothing_color).
    let mut groups: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (gender, color, attr_type, attr_value) in &rows {
        let gender_vn = gender_to_vn(gender);
        let color_vn = color.as_deref().map(color_to_vn).unwrap_or("không rõ màu");
        let key = format!("{gender_vn} áo {color_vn}");

        // Strip subject prefix (s0_, s1_) and map to Vietnamese.
        let stripped = attr_type
            .splitn(2, '_')
            .nth(1)
            .unwrap_or(attr_type.as_str());
        let label = match (stripped, attr_value.as_str()) {
            ("glasses", "yes") => Some("đeo kính"),
            ("hat", "yes") => Some("đội mũ"),
            ("mask", "yes") => Some("đeo khẩu trang"),
            ("bag", v) if v != "none" && v != "unknown" => Some(match v {
                "backpack" => "mang ba lô",
                "handbag" => "mang túi xách",
                _ => v,
            }),
            ("hair_color", v) if v != "unknown" => {
                // store as owned string; skip borrow issue by pushing a formatted string
                groups.entry(key.clone()).or_default().push(format!("tóc {}", color_to_vn(v)));
                continue;
            }
            ("clothing_type", v) if v != "unknown" => Some(clothing_type_to_vn(v)),
            ("interaction_type", v) => Some(match v {
                "physical_contact" => "tiếp xúc va chạm",
                "verbal" => "giao tiếp bằng lời",
                "gesture" => "ra hiệu cử chỉ",
                "proximity" => "ở cự ly gần",
                _ => v,
            }),
            ("age_group", "child") => Some("trẻ em"),
            ("age_group", "teen") => Some("thiếu niên"),
            ("age_group", "adult") => Some("người lớn"),
            ("age_group", "elderly") => Some("lớn tuổi"),
            _ => None,
        };
        if let Some(lbl) = label {
            groups.entry(key).or_default().push(lbl.to_string());
        }
    }

    groups
        .into_iter()
        .map(|(key, feats)| {
            let mut unique_feats = feats;
            unique_feats.dedup();
            format!("- Người {}: {}", key, unique_feats.join(", "))
        })
        .collect()
}


/// Simple offline answer when AI chat is disabled — extracts key facts from
/// the pre-built summary text via keyword matching.
fn fallback_answer_from_summary(summary: &str, question: &str) -> String {
    let q = question.to_lowercase();

    // Check for security / danger questions
    if q.contains("nguy hiểm") || q.contains("dangerous") || q.contains("đánh") || q.contains("an ninh") || q.contains("an toàn") {
        if let Some(sec) = extract_summary_section(summary, "2.") {
            return sec;
        }
        if let Some(sec) = extract_summary_section(summary, "1.") {
            return sec;
        }
    }
    // Check for counts
    if q.contains("bao nhiêu") || q.contains("how many") || q.contains("tổng") || q.contains("số lượng") || q.contains("mấy người") {
        if let Some(sec) = extract_summary_section(summary, "1.") {
            return sec;
        }
    }
    // Check for timeline / actions
    if q.contains("hành động") || q.contains("làm gì") || q.contains("diễn biến") || q.contains("timeline") || q.contains("khi nào") {
        if let Some(sec) = extract_summary_section(summary, "2.") {
            return sec;
        }
    }
    // Check for subjects / clothing / accessories
    if q.contains("người")
        || q.contains("đám đông")
        || q.contains("giới tính")
        || q.contains("áo")
        || q.contains("kính")
        || q.contains("mũ")
        || q.contains("khẩu trang")
        || q.contains("đeo")
        || q.contains("đội")
        || q.contains("tóc")
    {
        if let Some(sec) = extract_summary_section(summary, "3.") {
            return sec;
        }
    }

    // Generic: return the full summary.
    format!(
        "Dựa trên phân tích video:\n{summary}\n\n\
         (Bật AI để nhận câu trả lời chi tiết hơn cho câu hỏi này.)"
    )
}

fn extract_summary_section(summary: &str, marker: &str) -> Option<String> {
    let mut collecting = false;
    let mut lines = Vec::new();
    for line in summary.lines() {
        if line.contains(marker) {
            collecting = true;
            lines.push(line.trim());
            continue;
        }
        if collecting {
            let t = line.trim();
            if (t.starts_with("📌") || t.starts_with("⏱️") || t.starts_with("👤") || t.starts_with("🛡️")
                || (t.len() >= 2 && t.chars().next().map_or(false, |c| c.is_ascii_digit()) && t.chars().nth(1) == Some('.')))
                && !lines.is_empty()
            {
                break;
            }
            if !t.is_empty() {
                lines.push(t);
            }
        }
    }
    if !lines.is_empty() {
        Some(lines.join("\n"))
    } else {
        None
    }
}
