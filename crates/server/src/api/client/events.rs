use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}",
            put(send_event),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/state/{eventType}",
            put(send_state_event),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}",
            put(send_state_event_with_key),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/messages",
            get(get_messages),
        )
}

#[derive(Deserialize)]
struct SendEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "txnId")]
    txn_id: String,
}

async fn send_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<SendEventPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        None,
        &content,
    )
    .await?;

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

#[derive(Deserialize)]
struct StateEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
}

async fn send_state_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        Some(""),
        &content,
    )
    .await?;

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

#[derive(Deserialize)]
struct StateEventWithKeyPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "stateKey")]
    state_key: String,
}

async fn send_state_event_with_key(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventWithKeyPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        Some(&path.state_key),
        &content,
    )
    .await?;

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

async fn get_messages(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let events = db::events::get_messages(&state.pool, &room_id, 50).await?;
    Ok(Json(serde_json::json!({
        "chunk": events,
        "start": "",
        "end": "",
    })))
}
