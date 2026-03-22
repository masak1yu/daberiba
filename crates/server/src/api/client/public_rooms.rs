use crate::{error::ApiResult, state::AppState};
use axum::{extract::State, routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/publicRooms", get(get_public_rooms))
}

async fn get_public_rooms(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let rooms = db::rooms::get_public_rooms(&state.pool).await?;
    let chunk: Vec<serde_json::Value> = rooms
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "room_id": r.room_id,
                "name": r.name,
                "topic": r.topic,
                "num_joined_members": r.num_joined_members,
                "world_readable": false,
                "guest_can_join": false,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "chunk": chunk,
        "total_room_count_estimate": chunk.len(),
    })))
}
