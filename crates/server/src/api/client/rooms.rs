use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/createRoom", post(create_room))
        .route("/_matrix/client/v3/join/{roomIdOrAlias}", post(join_room))
        .route("/_matrix/client/v3/rooms/{roomId}/leave", post(leave_room))
        .route("/_matrix/client/v3/joined_rooms", get(joined_rooms))
}

#[derive(Deserialize)]
struct CreateRoomRequest {
    name: Option<String>,
    topic: Option<String>,
    _preset: Option<String>,
}

#[derive(Serialize)]
struct CreateRoomResponse {
    room_id: String,
}

async fn create_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<CreateRoomRequest>,
) -> ApiResult<Json<CreateRoomResponse>> {
    let room_id = db::rooms::create(
        &state.pool,
        &user.user_id,
        body.name.as_deref(),
        body.topic.as_deref(),
        &state.server_name,
    )
    .await?;

    Ok(Json(CreateRoomResponse { room_id }))
}

async fn join_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id_or_alias): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // エイリアス（#で始まる）の場合はルームIDに解決する
    let room_id = if room_id_or_alias.starts_with('#') {
        db::room_aliases::resolve(&state.pool, &room_id_or_alias)
            .await?
            .ok_or(AppError::NotFound)?
    } else {
        room_id_or_alias.clone()
    };

    db::rooms::join(&state.pool, &user.user_id, &room_id).await?;
    Ok(Json(serde_json::json!({ "room_id": room_id })))
}

async fn leave_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    db::rooms::leave(&state.pool, &user.user_id, &room_id).await?;
    Ok(Json(serde_json::json!({})))
}

async fn joined_rooms(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let rooms = db::rooms::joined_rooms(&state.pool, &user.user_id).await?;
    Ok(Json(serde_json::json!({ "joined_rooms": rooms })))
}
