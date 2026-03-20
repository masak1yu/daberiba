use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use crate::{error::{ApiResult, AppError}, middleware::auth::AuthUser, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/profile/{userId}", get(get_profile))
        .route(
            "/_matrix/client/v3/profile/{userId}/displayname",
            get(get_displayname).put(set_displayname),
        )
        .route(
            "/_matrix/client/v3/profile/{userId}/avatar_url",
            get(get_avatar_url).put(set_avatar_url),
        )
}

async fn get_profile(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let profile = db::profile::get(&state.pool, &user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(profile))
}

async fn get_displayname(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let profile = db::profile::get(&state.pool, &user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(serde_json::json!({
        "displayname": profile.get("displayname")
    })))
}

#[derive(Deserialize)]
struct SetDisplaynameBody {
    displayname: Option<String>,
}

async fn set_displayname(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(user_id): Path<String>,
    Json(body): Json<SetDisplaynameBody>,
) -> ApiResult<Json<serde_json::Value>> {
    if user.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    db::profile::set_displayname(&state.pool, &user_id, body.displayname.as_deref()).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_avatar_url(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let profile = db::profile::get(&state.pool, &user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(serde_json::json!({
        "avatar_url": profile.get("avatar_url")
    })))
}

#[derive(Deserialize)]
struct SetAvatarBody {
    avatar_url: Option<String>,
}

async fn set_avatar_url(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(user_id): Path<String>,
    Json(body): Json<SetAvatarBody>,
) -> ApiResult<Json<serde_json::Value>> {
    if user.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    db::profile::set_avatar_url(&state.pool, &user_id, body.avatar_url.as_deref()).await?;
    Ok(Json(serde_json::json!({})))
}
