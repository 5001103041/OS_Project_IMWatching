use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::{encode_access_token, hash_password, verify_password, AuthUser};
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

fn validate_password(pw: &str) -> AppResult<()> {
    if pw.len() < 6 {
        return Err(AppError::BadRequest(
            "password must be at least 6 characters".into(),
        ));
    }
    Ok(())
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// POST /api/auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(form): Json<RegisterForm>,
) -> AppResult<Json<Value>> {
    let email = normalize_email(&form.email);
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("valid email required".into()));
    }
    validate_password(&form.password)?;

    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest("email already registered".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password(&form.password)?;
    let role = if state.config.is_admin_email(&email) {
        "admin"
    } else {
        "member"
    };

    sqlx::query(
        "INSERT INTO users (id, email, name, password_hash, role) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&email)
    .bind(&form.name)
    .bind(&password_hash)
    .bind(role)
    .execute(&state.db)
    .await?;

    let (token, exp) = encode_access_token(&state, &id, role)?;
    Ok(Json(json!({
        "token": token,
        "expires_at": exp,
        "user": { "id": id, "email": email, "name": form.name, "role": role },
    })))
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(form): Json<LoginForm>,
) -> AppResult<Json<Value>> {
    let email = normalize_email(&form.email);
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized)?;

    if !verify_password(&form.password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }

    let (token, exp) = encode_access_token(&state, &user.id, &user.role)?;
    Ok(Json(json!({
        "token": token,
        "expires_at": exp,
        "user": { "id": user.id, "email": user.email, "name": user.name, "role": user.role },
    })))
}

/// GET /api/auth/me
pub async fn me(AuthUser(user): AuthUser) -> AppResult<Json<Value>> {
    Ok(Json(json!({
        "id": user.id,
        "email": user.email,
        "name": user.name,
        "role": user.role,
    })))
}
