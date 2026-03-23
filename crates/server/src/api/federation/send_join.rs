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
    let claims = crate::xmatrix::verify_request(&state, &headers, "PUT", &uri, Some(&body)).await?;

    crate::xmatrix::verify_pdu_signatures(&state, &body, &claims.origin)
        .await
        .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;

    let pdu_event_id = body["event_id"]
        .as_str()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing event_id".into()))?;
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
    let origin_server_ts = body["origin_server_ts"]
        .as_i64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    db::rooms::join(&state.pool, sender, &room_id).await?;

    let content = body["content"].clone();
    db::events::store_pdu(
        &state.pool,
        &db::events::PduMeta {
            event_id: pdu_event_id,
            room_id: &room_id,
            sender,
            event_type: "m.room.member",
            state_key: Some(sender),
            content: &content,
            origin_server_ts,
        },
    )
    .await?;

    // store_pdu 後の 4 クエリは互いに独立 — 並列実行
    let (state_events, auth_chain, members, room_version) = tokio::join!(
        db::room_state::get_all(&state.pool, &room_id),
        db::room_state::get_auth_events(&state.pool, &room_id),
        db::rooms::get_joined_members(&state.pool, &room_id),
        db::rooms::get_version(&state.pool, &room_id),
    );

    let state_events = state_events.unwrap_or_default();
    let auth_chain = auth_chain.unwrap_or_default();
    let servers_in_room: Vec<String> = members
        .unwrap_or_default()
        .keys()
        .filter_map(|uid| uid.split(':').nth(1).map(str::to_string))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let room_version = room_version
        .ok()
        .flatten()
        .unwrap_or_else(|| "10".to_string());

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    Ok(Json(serde_json::json!({
        "origin": server_name,
        "room_version": room_version,
        "auth_chain": auth_chain,
        "state": state_events,
        "servers_in_room": servers_in_room,
    })))
}
