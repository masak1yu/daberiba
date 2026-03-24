/// Federation make_join
/// GET /_matrix/federation/v1/make_join/{roomId}/{userId}
///
/// 他サーバーがルームに参加するための join イベントテンプレートを返す。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::get,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/federation/v1/make_join/:room_id/:user_id",
        get(make_join),
    )
}

async fn make_join(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "GET", &uri, None).await?;

    let (count, room_version, auth_event_ids, tip) = tokio::join!(
        db::rooms::count_joined_members(&state.pool, &room_id),
        db::rooms::get_version(&state.pool, &room_id),
        db::room_state::get_auth_event_ids(&state.pool, &room_id),
        db::events::get_room_tip(&state.pool, &room_id),
    );
    if count.unwrap_or(0) == 0 {
        return Err(crate::error::AppError::NotFound);
    }
    let room_version = room_version
        .ok()
        .flatten()
        .unwrap_or_else(|| "10".to_string());
    let auth_event_ids = auth_event_ids.unwrap_or_default();
    let (next_depth, prev_event_ids) = tip.unwrap_or((1, vec![]));

    let now_ms = chrono::Utc::now().timestamp_millis() as u64;

    // 参加イベントテンプレート（送信側が署名してから send_join に送る）
    let event = serde_json::json!({
        "type": "m.room.member",
        "room_id": room_id,
        "sender": user_id,
        "state_key": user_id,
        "content": { "membership": "join" },
        "origin": &*state.server_name,
        "origin_server_ts": now_ms,
        "auth_events": auth_event_ids,
        "prev_events": prev_event_ids,
        "depth": next_depth,
    });

    Ok(Json(serde_json::json!({
        "room_version": room_version,
        "event": event,
    })))
}
