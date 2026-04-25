use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/user/:userId/openid/request_token",
        post(request_token),
    )
}

/// POST /user/:userId/openid/request_token
/// Widget や外部サービスがユーザー身元を確認するための OpenID トークンを発行する。
/// 発行されたトークンは GET /_matrix/federation/v1/openid/userinfo で検証できる。
async fn request_token(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // 自分以外のトークンは発行不可
    if user_id != user.user_id {
        return Err(AppError::Forbidden);
    }

    let token = db::openid_tokens::create(&state.pool, &user.user_id).await?;

    Ok(Json(serde_json::json!({
        "access_token": token,
        "token_type": "Bearer",
        "matrix_server_name": *state.server_name,
        "expires_in": 3600,
    })))
}
