/// Federation send_join
/// PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}
///
/// 他サーバーから署名済み join イベントを受け取り、ルームに参加させる。
/// 完了後、現在のルームステートを返す。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::put,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/federation/v2/send_join/:room_id/:event_id",
        put(send_join),
    )
}

async fn send_join(
    State(state): State<AppState>,
    Path((room_id, _event_id)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "PUT", &uri, Some(&body)).await?;

    // PDU の基本フィールドを検証
    let sender = body["sender"]
        .as_str()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing sender".into()))?;
    let event_type = body["type"]
        .as_str()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing type".into()))?;
    if event_type != "m.room.member" {
        return Err(crate::error::AppError::BadRequest(
            "expected m.room.member event".into(),
        ));
    }
    let membership = body["content"]["membership"].as_str().unwrap_or("");
    if membership != "join" {
        return Err(crate::error::AppError::BadRequest(
            "expected membership: join".into(),
        ));
    }

    db::rooms::join(&state.pool, sender, &room_id).await?;

    let content = body["content"].clone();
    db::events::send(
        &state.pool,
        &room_id,
        sender,
        "m.room.member",
        Some(sender),
        &content,
    )
    .await?;

    let state_events = db::room_state::get_all(&state.pool, &room_id)
        .await
        .unwrap_or_default();

    let members = db::rooms::get_joined_members(&state.pool, &room_id)
        .await
        .unwrap_or_default();
    let servers_in_room: Vec<String> = members
        .keys()
        .filter_map(|uid| uid.split(':').nth(1).map(str::to_string))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    Ok(Json(serde_json::json!({
        "origin": server_name,
        "auth_chain": [],
        "state": state_events,
        "servers_in_room": servers_in_room,
    })))
}
