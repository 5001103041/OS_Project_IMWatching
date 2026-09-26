use std::env;
use std::path::PathBuf;

use sqlx::SqlitePool;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub media_dir: PathBuf,
    pub max_upload_bytes: usize,
    pub admin_emails: Vec<String>,
    /// AI vision config (analyzes frames — REQUIRES a multimodal model).
    pub ai_enabled: bool,
    pub ai_base_url: String,
    pub ai_api_key: String,
    pub ai_model: String,
    pub ai_timeout_secs: u64,
    /// AI chat config (chatbot — text-only, any model works).
    /// Falls back to the vision config when left empty.
    pub chat_enabled: bool,
    pub chat_base_url: String,
    pub chat_api_key: String,
    pub chat_model: String,
    /// CommandCode provider (a paid OpenAI-compatible aggregator). Keys for
    /// its preset models live here so they're selectable in the UI.
    pub commandcode_base_url: String,
    pub commandcode_api_key: String,
    /// FFmpeg binaries.
    pub ffmpeg_bin: String,
    pub ffprobe_bin: String,
    /// Frame extraction cadence (frames per second).
    pub frame_fps: f64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()?;
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://./data/imwatching.db?mode=rwc".to_string());
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_default();
        if jwt_secret.len() < 32 {
            anyhow::bail!(
                "JWT_SECRET must be set to a random string of at least 32 chars. \
                 Generate one with `openssl rand -hex 32` and add it to your .env."
            );
        }
        let media_dir = PathBuf::from(env::var("MEDIA_DIR").unwrap_or_else(|_| "./media".to_string()));
        let max_upload_bytes = env::var("MAX_UPLOAD_BYTES")
            .unwrap_or_else(|_| "209715200".to_string()) // 200 MiB
            .parse()
            .unwrap_or(209_715_200);
        let admin_emails: Vec<String> = env::var("ADMIN_EMAILS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let ai_enabled = env::var("AI_ENABLED")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"))
            .unwrap_or(false);
        let ai_base_url = env::var("AI_BASE_URL").unwrap_or_default();
        let ai_api_key = env::var("AI_API_KEY").unwrap_or_default();
        let ai_model = env::var("AI_MODEL").unwrap_or_else(|_| "gemini-3.8-flash".to_string());
        let ai_timeout_secs = env::var("AI_TIMEOUT_SECS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60);

        // Chat (chatbot) config. Independent from vision so a cheap text-only
        // model can power the chatbot while a multimodal model analyzes frames.
        // Empty values fall back to the vision config.
        let chat_base_url = match env::var("CHAT_BASE_URL") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => ai_base_url.clone(),
        };
        let chat_api_key = match env::var("CHAT_API_KEY") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => ai_api_key.clone(),
        };
        let chat_model = match env::var("CHAT_MODEL") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => ai_model.clone(),
        };
        // Chat is enabled when the vision switch is on, unless CHAT_ENABLED
        // explicitly overrides it.
        let chat_enabled = env::var("CHAT_ENABLED")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"))
            .unwrap_or(ai_enabled);

        let commandcode_base_url = env::var("COMMANDCODE_BASE_URL").unwrap_or_default();
        let commandcode_api_key = env::var("COMMANDCODE_API_KEY").unwrap_or_default();

        let ffmpeg_bin = env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".to_string());
        let ffprobe_bin = env::var("FFPROBE_BIN").unwrap_or_else(|_| "ffprobe".to_string());
        let frame_fps = env::var("FRAME_FPS")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .unwrap_or(1.0);

        Ok(Self {
            port,
            database_url,
            jwt_secret,
            media_dir,
            max_upload_bytes,
            admin_emails,
            ai_enabled,
            ai_base_url,
            ai_api_key,
            ai_model,
            ai_timeout_secs,
            chat_enabled,
            chat_base_url,
            chat_api_key,
            chat_model,
            commandcode_base_url,
            commandcode_api_key,
            ffmpeg_bin,
            ffprobe_bin,
            frame_fps,
        })
    }

    pub fn is_admin_email(&self, email: &str) -> bool {
        let lc = email.to_lowercase();
        self.admin_emails.iter().any(|e| e == &lc)
    }
}

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            config,
            db,
            http_client,
        }
    }
}
