use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/register", post(register))
        .route(
            "/_matrix/client/v3/register/available",
            get(register_available),
        )
        .route("/_matrix/client/v3/login", get(login_flows).post(login))
}

/// 認証必須ルート（router.rs 側で auth middleware を付けて登録する）
pub fn protected_routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v1/login/get_token", post(get_login_token))
}

async fn login_flows() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "flows": [
            { "type": "m.login.password" },
            { "type": "m.login.token" }
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

    let (user_id, access_token, device_id) = db::users::register(
        &state.pool,
        &body.username,
        &body.password,
        &state.server_name,
    )
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
    token: Option<String>,
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
    match body.login_type.as_str() {
        "m.login.password" => {
            let username = body
                .identifier
                .and_then(|id| id.user)
                .ok_or_else(|| AppError::BadRequest("missing identifier.user".into()))?;

            let password = body
                .password
                .ok_or_else(|| AppError::BadRequest("missing password".into()))?;

            let (user_id, access_token, device_id) =
                db::users::login(&state.pool, &username, &password, &state.server_name)
                    .await
                    .map_err(|_| AppError::Unauthorized)?;

            Ok(Json(LoginResponse {
                user_id,
                access_token,
                device_id,
            }))
        }
        "m.login.token" => {
            let token = body
                .token
                .ok_or_else(|| AppError::BadRequest("missing token".into()))?;

            // トークンを消費して user_id を取得
            let user_id = db::login_tokens::consume(&state.pool, &token)
                .await
                .map_err(AppError::Internal)?
                .ok_or(AppError::Unauthorized)?;

            // 新しいデバイスとアクセストークンを発行
            let device_id = uuid::Uuid::new_v4()
                .to_string()
                .replace('-', "")
                .to_uppercase();
            let device_id = format!("DEVICE_{}", &device_id[..8]);
            db::devices::create(&state.pool, &user_id, &device_id)
                .await
                .map_err(AppError::Internal)?;
            let access_token = db::access_tokens::create(&state.pool, &user_id, &device_id)
                .await
                .map_err(AppError::Internal)?;

            Ok(Json(LoginResponse {
                user_id,
                access_token,
                device_id,
            }))
        }
        _ => Err(AppError::BadRequest("unsupported login type".into())),
    }
}

/// POST /_matrix/client/v1/login/get_token
/// 現在のセッションから短命ログイントークンを発行する（クロスデバインログイン用）。
async fn get_login_token(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let token = db::login_tokens::create(&state.pool, &user.user_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(serde_json::json!({
        "login_token": token,
        "expires_in_ms": 120_000,
    })))
}

#[derive(Deserialize)]
struct RegisterAvailableQuery {
    username: Option<String>,
}

/// GET /_matrix/client/v3/register/available?username=<localpart>
/// ユーザー名が利用可能か確認する。利用可能なら { available: true }。
async fn register_available(
    State(state): State<AppState>,
    Query(params): Query<RegisterAvailableQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let localpart = params
        .username
        .ok_or_else(|| AppError::BadRequest("username required".into()))?;

    // ローカルパートを user_id 形式に変換
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let user_id = format!("@{}:{}", localpart, server_name);

    let exists = db::users::exists(&state.pool, &user_id).await?;
    if exists {
        return Err(AppError::BadRequest("M_USER_IN_USE".into()));
    }

    Ok(Json(serde_json::json!({ "available": true })))
}
