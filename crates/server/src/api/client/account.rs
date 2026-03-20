use axum::{extract::State, routing::{get, post}, Json, Router};
use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/account/whoami", get(whoami))
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
