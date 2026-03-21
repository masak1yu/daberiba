use crate::{
    error::{ApiResult, AppError},
    state::AppState,
};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/register", post(register))
        .route("/_matrix/client/v3/login", get(login_flows).post(login))
}

async fn login_flows() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "flows": [
            { "type": "m.login.password" }
        ]
    }))
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    user_id: String,
    access_token: String,
    device_id: String,
}

fn validate_username(username: &str) -> Result<(), AppError> {
    if username.is_empty() {
        return Err(AppError::BadRequest("username must not be empty".into()));
    }
    if username.len() > 255 {
        return Err(AppError::BadRequest("username too long".into()));
    }
    // Matrix localpart: 英数字・アンダースコア・ハイフン・ドットのみ
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(AppError::BadRequest(
            "username may only contain a-z, 0-9, _, -, .".into(),
        ));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Json<RegisterResponse>> {
    validate_username(&body.username)?;
    validate_password(&body.password)?;

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let (user_id, access_token, device_id) =
        db::users::register(&state.pool, &body.username, &body.password, &server_name)
            .await
            .map_err(|e| {
                // 重複ユーザー
                if e.to_string().contains("Duplicate") || e.to_string().contains("duplicate") {
                    AppError::BadRequest("username already taken".into())
                } else {
                    AppError::Internal(e)
                }
            })?;

    Ok(Json(RegisterResponse {
        user_id,
        access_token,
        device_id,
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    #[serde(rename = "type")]
    login_type: String,
    identifier: Option<LoginIdentifier>,
    password: Option<String>,
}

#[derive(Deserialize)]
struct LoginIdentifier {
    #[serde(rename = "type")]
    _id_type: String,
    user: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    user_id: String,
    access_token: String,
    device_id: String,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    if body.login_type != "m.login.password" {
        return Err(AppError::BadRequest("unsupported login type".into()));
    }

    let username = body
        .identifier
        .and_then(|id| id.user)
        .ok_or_else(|| AppError::BadRequest("missing identifier.user".into()))?;

    let password = body
        .password
        .ok_or_else(|| AppError::BadRequest("missing password".into()))?;

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let (user_id, access_token, device_id) =
        db::users::login(&state.pool, &username, &password, &server_name)
            .await
            .map_err(|_| AppError::Unauthorized)?;

    Ok(Json(LoginResponse {
        user_id,
        access_token,
        device_id,
    }))
}
