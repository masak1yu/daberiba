use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
    uia,
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/account/whoami", get(whoami))
        .route("/_matrix/client/v3/account/password", post(change_password))
        .route(
            "/_matrix/client/v3/account/deactivate",
            post(deactivate_account),
        )
        .route("/_matrix/client/v3/logout", post(logout))
        .route("/_matrix/client/v3/logout/all", post(logout_all))
}

async fn whoami(
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "user_id": user.user_id,
        "device_id": user.device_id,
    })))
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    new_password: String,
    #[serde(default)]
    auth: Option<serde_json::Value>,
}

async fn change_password(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<ChangePasswordBody>,
) -> impl IntoResponse {
    // auth がない → UIA チャレンジ
    let auth = match &body.auth {
        Some(a) => a,
        None => return state.uia.challenge().into_response(),
    };

    // session 検証
    let session = auth.get("session").and_then(|v| v.as_str()).unwrap_or("");
    if !state.uia.validate(session) {
        return state.uia.challenge().into_response();
    }

    // type が m.login.password でない → チャレンジ
    let password = match uia::extract_password(auth) {
        Some(p) => p.to_string(),
        None => return state.uia.challenge().into_response(),
    };

    match db::users::change_password(&state.pool, &user.user_id, &password, &body.new_password)
        .await
    {
        Ok(()) => Json(serde_json::json!({})).into_response(),
        Err(_) => AppError::Forbidden.into_response(),
    }
}

#[derive(Deserialize)]
struct DeactivateBody {
    #[serde(default)]
    auth: Option<serde_json::Value>,
}

/// POST /_matrix/client/v3/account/deactivate
/// アカウントを無効化する。全トークンを失効させ、パスワードを無効化する。
async fn deactivate_account(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<DeactivateBody>,
) -> impl IntoResponse {
    // auth がない → UIA チャレンジ
    let auth = match &body.auth {
        Some(a) => a,
        None => return state.uia.challenge().into_response(),
    };

    let session = auth.get("session").and_then(|v| v.as_str()).unwrap_or("");
    if !state.uia.validate(session) {
        return state.uia.challenge().into_response();
    }

    let password = match uia::extract_password(auth) {
        Some(p) => p.to_string(),
        None => return state.uia.challenge().into_response(),
    };

    // パスワード確認
    match db::users::verify(&state.pool, &user.user_id, &password).await {
        Ok(true) => {}
        _ => return AppError::Forbidden.into_response(),
    }

    // 全トークン失効 + アカウント無効化
    let _ = db::access_tokens::revoke_all(&state.pool, &user.user_id).await;
    let _ = db::users::deactivate(&state.pool, &user.user_id).await;

    Json(serde_json::json!({ "id_server_unbind_result": "no-support" })).into_response()
}

async fn logout(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    db::access_tokens::revoke(&state.pool, &user.token).await?;
    Ok(Json(serde_json::json!({})))
}

async fn logout_all(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    db::access_tokens::revoke_all(&state.pool, &user.user_id).await?;
    Ok(Json(serde_json::json!({})))
}
