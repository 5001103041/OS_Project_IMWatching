//! Optional AI analysis of extracted video frames.
//!
//! Speaks OpenAI-compatible `POST /chat/completions` with a base64 image part.
//! Any error yields Skipped/Failed so the pipeline always continues.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ─── System prompt ───────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "\
You are a video-frame behavior analyzer for a surveillance/education project.

You receive ONE image (a single video frame). Analyze EVERY visible person and the scene. \
Return ONLY a valid JSON object, no markdown, no prose. The object must have EXACTLY this shape:

{
  \"scene\": {
    \"location\": \"indoor|outdoor|unknown\",
    \"lighting\": \"bright|dim|dark|unknown\",
    \"time_of_day\": \"day|night|unknown\",
    \"crowd_density\": \"empty|sparse|moderate|crowded|unknown\",
    \"environment\": \"street|classroom|office|park|parking_lot|home|store|unknown\"
  },
  \"behaviors\": [
    {
      \"behavior_name\": \"walking|standing|sitting|studying|talking|fighting|school_violence|no_hands_driving|running|falling|carrying_object|phone_use|smoking|hugging|dancing|...\",
      \"severity\": \"normal|dangerous\",
      \"confidence\": 0.0,
      \"interaction_type\": \"physical_contact|verbal|gesture|proximity|none\",
      \"reason\": \"short explanation in Vietnamese\",
      \"subjects\": [
        {
          \"role\": \"aggressor|victim|bystander|participant|unknown\",
          \"gender\": \"male|female|unknown\",
          \"clothing_color\": \"a main color word (red, white, black, blue, ...) or unknown\",
          \"clothing_type\": \"shirt|hoodie|dress|uniform|jacket|t-shirt|sweater|tank-top|unknown\",
          \"glasses\": true,
          \"hat\": false,
          \"bag\": \"backpack|handbag|none|unknown\",
          \"age_group\": \"child|teen|adult|elderly|unknown\",
          \"mask\": false,
          \"hair_color\": \"black|brown|blonde|gray|red|unknown\",
          \"confidence\": 0.0
        }
      ]
    }
  ]
}

Rules:
- ALWAYS include the scene object with your best assessment.
- List EVERY person you can see. If there are 5 people, subjects MUST contain up to 5 entries (one per person).
- behavior_name: only lowercase_snake_case. Examples: walking, standing, sitting, studying, talking, gesturing, running, fighting, school_violence, no_hands_driving, falling, carrying_object, phone_use, smoking.
- severity rules (CRITICAL ACCURACY):
  * 'normal': Everyday activities MUST ALWAYS be 'normal'. This strictly includes:
    walking (đi bộ), gesturing (ra hiệu, vẫy tay, giơ tay, làm dấu chữ V, chỉ tay), talking (nói chuyện, trò chuyện), standing (đứng), sitting (ngồi), studying (học tập), running (chạy bộ), phone_use (dùng điện thoại), carrying_object (mang vác đồ vật), drinking, eating, dancing, hugging.
    UNDER NO CIRCUMSTANCES should 'walking', 'gesturing', 'talking', 'standing', or 'sitting' be marked as 'dangerous'!
  * 'dangerous': ONLY for real physical violence, crimes, life threats, or dangerous accidents:
    fighting (đánh nhau, ẩu đả, xô xát, đấm đá, vật lộn), school_violence (bạo lực học đường), assault (tấn công, hành hung), weapon_use (dùng vũ khí, dao, súng, gậy), falling (té ngã chấn thương, tai nạn), no_hands_driving (lái xe buông cả 2 tay), robbery/theft (trộm cướp), vandalism (phá hoại tài sản).
- clothing_type MUST strictly distinguish between 't-shirt' and 'shirt':
  * 't-shirt' (áo thun, áo phông): ALL casual tops, stretch knit tops, athletic shirts, sportswear, crewneck/round-neck tops, V-neck tops, plain tops, graphic tees, or any pullover tops without buttons. In home, school, or casual street settings, everyday tops are 't-shirt'. When in doubt or if buttons are absent, ALWAYS choose 't-shirt'.
  * 'shirt' (áo sơ mi): ONLY formal or office woven dress shirts with an explicit structured turn-down collar AND a full vertical row of buttons/cúc running down the center front (button-up/down shirt). NEVER use 'shirt' for casual tops, athletic tops, or round-collar tops! If there are no clear buttons down the chest, it MUST be 't-shirt'.
  * 'hoodie': hooded sweatshirt with or without hood up.
  * 'jacket': zip-up or buttoned outerwear jacket, windbreaker, or coat.
  * 'dress': female one-piece dress or skirt dress.
  * 'uniform': school uniform or formal corporate uniform.
  * 'sweater': knitted wool or heavy knit sweater.
  * 'tank-top': sleeveless undershirt or vest.
- interaction_type: use 'physical_contact' for fighting/hugging, 'verbal' for talking/shouting, 'gesture' for waving/pointing/peace-sign, 'proximity' for standing close, 'none' for solo activities.
- role: in multi-person interactions, identify who is the aggressor/victim/participant. Use 'unknown' if unclear.
- Every subject MUST include gender and clothing_color.
- For optional fields (glasses, hat, mask): use true/false only when clearly visible; omit the field entirely if unsure.
- clothing_type, bag, age_group, hair_color: use \"unknown\" when not visible.
- If no person is visible return {\"behaviors\": []}.
- Do not invent information not visible in the frame.
- The whole response must be parseable JSON (no trailing commas, no comments).
- Be accurate and precise — accuracy matters more than speed.
";

// ─── Data types ──────────────────────────────────────────────────────────────

/// One subject detected inside a behavior event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiSubject {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub gender: String,
    #[serde(default)]
    pub clothing_color: String,

    // ── Extended attributes (all optional / backward-compatible) ─────────────
    #[serde(default)]
    pub clothing_type: Option<String>,
    #[serde(default)]
    pub glasses: Option<bool>,
    #[serde(default)]
    pub hat: Option<bool>,
    #[serde(default)]
    pub bag: Option<String>,
    #[serde(default)]
    pub age_group: Option<String>,
    #[serde(default)]
    pub mask: Option<bool>,
    #[serde(default)]
    pub hair_color: Option<String>,

    #[serde(default)]
    pub confidence: f32,
}

impl AiSubject {
    /// Produce a list of `(attribute_type, attribute_value)` pairs for every
    /// non-trivial extended attribute, ready to insert into `event_attributes`.
    pub fn extended_attrs(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();

        if let Some(ct) = &self.clothing_type {
            let v = ct.trim().to_lowercase();
            if !v.is_empty() && v != "unknown" {
                out.push(("clothing_type", v));
            }
        }
        if let Some(g) = self.glasses {
            out.push(("glasses", if g { "yes" } else { "no" }.to_string()));
        }
        if let Some(h) = self.hat {
            out.push(("hat", if h { "yes" } else { "no" }.to_string()));
        }
        if let Some(b) = &self.bag {
            let v = b.trim().to_lowercase();
            if !v.is_empty() && v != "unknown" && v != "none" {
                out.push(("bag", v));
            }
        }
        if let Some(ag) = &self.age_group {
            let v = ag.trim().to_lowercase();
            if !v.is_empty() && v != "unknown" {
                out.push(("age_group", v));
            }
        }
        if let Some(m) = self.mask {
            out.push(("mask", if m { "yes" } else { "no" }.to_string()));
        }
        if let Some(hc) = &self.hair_color {
            let v = hc.trim().to_lowercase();
            if !v.is_empty() && v != "unknown" {
                out.push(("hair_color", v));
            }
        }

        out
    }
}

/// One behavior detected in a frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiBehavior {
    #[serde(default)]
    pub behavior_name: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub interaction_type: Option<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub subjects: Vec<AiSubject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiScene {
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub lighting: Option<String>,
    #[serde(default)]
    pub time_of_day: Option<String>,
    #[serde(default)]
    pub crowd_density: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
}

/// The structured verdict parsed from the AI response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiVerdict {
    #[serde(default)]
    pub behaviors: Vec<AiBehavior>,
    #[serde(default)]
    pub scene: Option<AiScene>,
}

/// Outcome of an analysis attempt.
pub enum AiOutcome {
    /// AI ran and returned a parseable verdict.
    Ok(AiVerdict),
    /// AI ran but the response could not be parsed.
    Failed,
    /// AI is disabled/misconfigured or a transport error occurred.
    Skipped,
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: Vec<ContentPart>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
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
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Analyze a single JPEG frame. Returns an `Ok` verdict on success,
/// `Failed` on unparseable response, `Skipped` when AI is disabled.
pub async fn analyze_frame(state: &AppState, jpeg: &[u8]) -> AiOutcome {
    let cfg = crate::ai_config::resolve_vision(state).await;
    if !cfg.enabled || cfg.base_url.trim().is_empty() {
        return AiOutcome::Skipped;
    }

    let user_content = vec![ContentPart::ImageUrl {
        image_url: ImageUrl {
            url: format!("data:image/jpeg;base64,{}", base64_encode(jpeg)),
        },
    }];

    let request = ChatRequest {
        model: cfg.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system",
                content: vec![ContentPart::Text {
                    text: SYSTEM_PROMPT.to_string(),
                }],
            },
            ChatMessage {
                role: "user",
                content: user_content,
            },
        ],
        // Increased to 1000 to accommodate extended subject attributes.
        max_tokens: Some(1000),
        temperature: Some(0.0),
    };

    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(state.config.ai_timeout_secs.max(5)))
        .build()
    {
        Ok(c) => c,
        Err(_) => return AiOutcome::Failed,
    };

    let resp = match send_with_retry(&client, &endpoint, &request, &cfg.api_key).await {
        Some(r) => r,
        None => return AiOutcome::Failed,
    };

    let body: ChatResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("ai: parse response: {e}");
            return AiOutcome::Failed;
        }
    };

    let Some(choice) = body.choices.into_iter().next() else {
        return AiOutcome::Failed;
    };

    match parse_verdict_json(&choice.message.content) {
        Some(v) => {
            tracing::info!("ai: ok, {} behavior(s) detected", v.behaviors.len());
            AiOutcome::Ok(v)
        }
        None => {
            tracing::warn!("ai: unparseable verdict: {}", choice.message.content);
            AiOutcome::Failed
        }
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

async fn send_with_retry(
    client: &reqwest::Client,
    endpoint: &str,
    request: &ChatRequest,
    api_key: &str,
) -> Option<reqwest::Response> {
    const MAX_ATTEMPTS: u32 = 6;
    let mut delay_ms: u64 = 2000;

    for attempt in 1..=MAX_ATTEMPTS {
        let mut req = client.post(endpoint).json(request);
        if !api_key.trim().is_empty() {
            req = req.bearer_auth(api_key);
        }

        match req.send().await {
            Ok(r) if r.status().is_success() => return Some(r),
            Ok(r) => {
                let status = r.status();
                let retryable = status.as_u16() == 429 || status.is_server_error();
                if !retryable {
                    let body = r.text().await.unwrap_or_default();
                    tracing::warn!("ai: non-success status {status}, body: {body}");
                    return None;
                }
                if attempt == MAX_ATTEMPTS {
                    let body = r.text().await.unwrap_or_default();
                    tracing::warn!(
                        "ai: giving up after {MAX_ATTEMPTS} attempts, status {status}, body: {body}"
                    );
                    return None;
                }
                tracing::warn!(
                    "ai: attempt {attempt}/{MAX_ATTEMPTS} got {status}, retrying in {delay_ms}ms"
                );
            }
            Err(e) => {
                if attempt == MAX_ATTEMPTS {
                    tracing::warn!("ai: giving up after {MAX_ATTEMPTS} attempts: {e}");
                    return None;
                }
                tracing::warn!("ai: attempt {attempt}/{MAX_ATTEMPTS} transport error: {e}");
            }
        }

        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        delay_ms = (delay_ms * 2).min(20_000);
    }
    None
}

/// Tolerant JSON extraction: find the first balanced `{...}` and parse it.
fn parse_verdict_json(text: &str) -> Option<AiVerdict> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
