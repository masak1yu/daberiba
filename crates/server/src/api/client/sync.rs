use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/sync", get(sync))
}

#[derive(Deserialize)]
struct SyncQuery {
    since: Option<String>,
    #[allow(dead_code)]
    timeout: Option<u64>,
    #[allow(dead_code)]
    filter: Option<String>,
}

async fn sync(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<SyncQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut result = db::sync::sync(&state.pool, &user.user_id, query.since.as_deref()).await?;

    // 各参加ルームに ephemeral イベント（m.typing / m.receipt）を付加
    if let Some(join_map) = result
        .get_mut("rooms")
        .and_then(|r| r.get_mut("join"))
        .and_then(|j| j.as_object_mut())
    {
        for (room_id, room_data) in join_map.iter_mut() {
            let mut ephemeral_events: Vec<serde_json::Value> = Vec::new();

            // m.typing
            let typing_users = state.typing.get_typing(room_id);
            ephemeral_events.push(serde_json::json!({
                "type": "m.typing",
                "content": { "user_ids": typing_users },
            }));

            // m.receipt
            if let Ok(receipts) = db::receipts::get_for_room(&state.pool, room_id).await {
                if !receipts.is_empty() {
                    ephemeral_events.push(db::receipts::to_event(receipts));
                }
            }

            if let Some(ephemeral) = room_data.get_mut("ephemeral") {
                if let Some(events) = ephemeral.get_mut("events") {
                    *events = serde_json::json!(ephemeral_events);
                }
            }
        }
    }

    Ok(Json(result))
}
