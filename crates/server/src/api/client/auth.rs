use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::{error::ApiResult, state::AppState};

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

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Json<RegisterResponse>> {
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let (user_id, access_token, device_id) =
        db::users::register(&state.pool, &body.username, &body.password, &server_name).await?;

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
    id_type: String,
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
        return Err(crate::error::AppError::BadRequest(
            "unsupported login type".to_string(),
        ));
    }

    let username = body
        .identifier
        .and_then(|id| id.user)
        .ok_or_else(|| crate::error::AppError::BadRequest("missing identifier".to_string()))?;

    let password = body
        .password
        .ok_or_else(|| crate::error::AppError::BadRequest("missing password".to_string()))?;

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let (user_id, access_token, device_id) =
        db::users::login(&state.pool, &username, &password, &server_name).await?;

    Ok(Json(LoginResponse {
        user_id,
        access_token,
        device_id,
    }))
}
