use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/rooms/{roomId}/state", get(get_state))
        .route(
            "/_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}",
            get(get_state_event),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/members",
            get(get_members),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/joined_members",
            get(get_joined_members),
        )
        .route("/_matrix/client/v3/rooms/{roomId}/invite", post(invite))
}

async fn get_state(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let events = db::room_state::get_all(&state.pool, &room_id).await?;
    Ok(Json(serde_json::json!(events)))
}

#[derive(Deserialize)]
struct StateEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "stateKey")]
    state_key: String,
}

async fn get_state_event(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventPath>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = db::room_state::get_event(
        &state.pool,
        &path.room_id,
        &path.event_type,
        &path.state_key,
    )
    .await?
    .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(content))
}

async fn get_members(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let members = db::rooms::get_members(&state.pool, &room_id).await?;
    Ok(Json(serde_json::json!({ "chunk": members })))
}

async fn get_joined_members(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let members = db::rooms::get_joined_members(&state.pool, &room_id).await?;
    Ok(Json(serde_json::json!({ "joined": members })))
}

#[derive(Deserialize)]
struct InviteBody {
    user_id: String,
}

async fn invite(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<InviteBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::rooms::invite(&state.pool, &room_id, &user.user_id, &body.user_id).await?;
    Ok(Json(serde_json::json!({})))
}
