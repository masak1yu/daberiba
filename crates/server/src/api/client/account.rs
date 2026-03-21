use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/account/whoami", get(whoami))
        .route("/_matrix/client/v3/account/password", post(change_password))
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
) -> ApiResult<Json<serde_json::Value>> {
    // auth.password から現在のパスワードを取得
    let old_password = body
        .auth
        .as_ref()
        .and_then(|a| a.get("password"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("auth.password is required".into()))?;

    db::users::change_password(&state.pool, &user.user_id, old_password, &body.new_password)
        .await
        .map_err(|_| AppError::Forbidden)?;

    Ok(Json(serde_json::json!({})))
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
