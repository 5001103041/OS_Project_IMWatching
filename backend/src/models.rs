use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn is_admin_bool(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Video {
    pub id: String,
    pub owner_id: String,
    pub title: Option<String>,
    pub original_filename: String,
    pub final_filename: Option<String>,
    pub behavior_category: Option<String>,
    pub storage_path: String,
    pub mime_type: String,
    pub duration_secs: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub file_size_bytes: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VideoFrame {
    pub id: String,
    pub video_id: String,
    pub frame_index: i64,
    pub timestamp_secs: f64,
    pub storage_path: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub analyzed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DetectedEvent {
    pub id: String,
    pub video_id: String,
    pub frame_id: Option<String>,
    pub timestamp_secs: f64,
    #[serde(default)]
    pub end_timestamp_secs: Option<f64>,
    pub behavior_name: String,
    pub severity: String,
    pub gender: String,
    pub clothing_color: Option<String>,
    pub confidence: f64,
    pub ai_reason: Option<String>,
    #[serde(default)]
    pub subjects_json: Option<String>,
    pub frame_image_path: String,
    pub review_status: String,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventAttribute {
    pub id: String,
    pub event_id: String,
    pub attribute_type: String,
    pub attribute_value: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: String,
    pub user_id: String,
    pub video_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
