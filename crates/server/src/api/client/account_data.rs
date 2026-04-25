use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::put,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        // グローバル account_data
        .route(
            "/_matrix/client/v3/user/:userId/account_data/:type",
            put(set_global).get(get_global),
        )
        // ルーム固有 account_data
        .route(
            "/_matrix/client/v3/user/:userId/rooms/:roomId/account_data/:type",
            put(set_room).get(get_room),
        )
}

async fn set_global(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((_, event_type)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = serde_json::to_string(&body).unwrap_or_default();
    db::account_data::set(&state.pool, &user.user_id, "", &event_type, &content).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_global(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((_, event_type)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = db::account_data::get(&state.pool, &user.user_id, "", &event_type)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(content))
}

async fn set_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((_, room_id, event_type)): Path<(String, String, String)>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = serde_json::to_string(&body).unwrap_or_default();
    db::account_data::set(&state.pool, &user.user_id, &room_id, &event_type, &content).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((_, room_id, event_type)): Path<(String, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = db::account_data::get(&state.pool, &user.user_id, &room_id, &event_type)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(content))
}
