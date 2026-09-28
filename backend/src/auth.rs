use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use hmac::{Hmac, Mac};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::AppError;
use crate::models::User;
use crate::state::AppState;

pub const ACCESS_TOKEN_TTL_SECS: i64 = 3600 * 24 * 30; // 30 days

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String, // user id
    pub role: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

pub fn encode_access_token(
    state: &AppState,
    user_id: &str,
    role: &str,
) -> Result<(String, i64), AppError> {
    let now = chrono::Utc::now().timestamp();
    let exp = now + ACCESS_TOKEN_TTL_SECS;
    let claims = AccessClaims {
        sub: user_id.to_string(),
        role: role.to_string(),
        iat: now,
        exp,
        jti: uuid::Uuid::new_v4().to_string(),
    };
    let key = EncodingKey::from_secret(state.config.jwt_secret.as_bytes());
    let token = jsonwebtoken::encode(&Header::default(), &claims, &key)
        .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))?;
    Ok((token, exp))
}

pub fn decode_access_token(state: &AppState, token: &str) -> Result<AccessClaims, AppError> {
    let key = DecodingKey::from_secret(state.config.jwt_secret.as_bytes());
    let mut validation = Validation::default();
    validation.leeway = 5;
    let data = jsonwebtoken::decode::<AccessClaims>(token, &key, &validation)
        .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;

/// Hash a plaintext password using PBKDF2-HMAC-SHA256.
/// Format: `pbkdf2_sha256$<iterations>$<salt_b64>$<hash_b64>`
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let mut salt = [0u8; SALT_LEN];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut salt);

    let hash = pbkdf2(password.as_bytes(), &salt, PBKDF2_ITERATIONS);
    Ok(format!(
        "pbkdf2_sha256${}${}${}",
        PBKDF2_ITERATIONS,
        base64::engine::general_purpose::STANDARD.encode(salt),
        base64::engine::general_purpose::STANDARD.encode(hash)
    ))
}

/// Verify a plaintext password against a stored PBKDF2 hash.
pub fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    let Ok(iterations) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(salt) = base64::engine::general_purpose::STANDARD.decode(parts[2]) else {
        return false;
    };
    let Ok(expected) = base64::engine::general_purpose::STANDARD.decode(parts[3]) else {
        return false;
    };
    let actual = pbkdf2(password.as_bytes(), &salt, iterations);
    // Constant-time comparison.
    if actual.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in actual.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn pbkdf2(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(password).expect("HMAC accepts any key length");
    let mut result = [0u8; 32];
    // PBKDF2 with a single 32-byte block (dkLen = 32).
    let mut block = salt.to_vec();
    block.extend_from_slice(&1u32.to_be_bytes());

    let mut u = [0u8; 32];
    mac.update(&block);
    let out = mac.finalize().into_bytes();
    u.copy_from_slice(&out);
    result.copy_from_slice(&out);

    for _ in 1..iterations {
        let mut m = HmacSha256::new_from_slice(password).expect("HMAC accepts any key length");
        m.update(&u);
        let out = m.finalize().into_bytes();
        u.copy_from_slice(&out);
        for (r, uu) in result.iter_mut().zip(u.iter()) {
            *r ^= uu;
        }
    }
    result
}

/// Extractor for a Bearer-JWT-authenticated user.
#[derive(Clone, Debug)]
pub struct AuthUser(pub User);

#[axum::async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v.to_string());

        let Some(token) = token else {
            return Err((StatusCode::UNAUTHORIZED, "missing bearer token").into_response());
        };

        let claims = decode_access_token(state, &token)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token").into_response())?;

        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(&claims.sub)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::Internal(e.to_string()).into_response())?;

        match user {
            Some(u) => Ok(AuthUser(u)),
            None => Err((StatusCode::UNAUTHORIZED, "user gone").into_response()),
        }
    }
}

/// Extractor for admin-only routes.
#[derive(Clone, Debug)]
pub struct AdminUser(pub User);

#[axum::async_trait]
impl FromRequestParts<Arc<AppState>> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(u) = AuthUser::from_request_parts(parts, state).await?;
        if !u.is_admin_bool() {
            return Err((StatusCode::FORBIDDEN, "admin only").into_response());
        }
        Ok(AdminUser(u))
    }
}
