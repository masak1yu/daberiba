use crate::{
    error::{ApiResult, AppError},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/federation/v1/openid/userinfo", get(userinfo))
}

#[derive(Deserialize)]
struct UserInfoQuery {
    access_token: Option<String>,
}

/// GET /_matrix/federation/v1/openid/userinfo?access_token=<token>
/// 外部サービスが OpenID トークンを検証して Matrix user_id を取得するエンドポイント。
async fn userinfo(
    State(state): State<AppState>,
    Query(query): Query<UserInfoQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let token = query.access_token.ok_or(AppError::Unauthorized)?;
    let user_id = db::openid_tokens::verify(&state.pool, &token)
        .await?
        .ok_or(AppError::Unauthorized)?;

    Ok(Json(serde_json::json!({ "sub": user_id })))
}
