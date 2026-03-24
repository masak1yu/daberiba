/// Federation send_leave
/// PUT /_matrix/federation/v2/send_leave/{roomId}/{eventId}
///
/// 他サーバーから署名済み leave イベントを受け取り、ルームから退出させる。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::put,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/federation/v2/send_leave/:room_id/:event_id",
        put(send_leave),
    )
}

async fn send_leave(
    State(state): State<AppState>,
    Path((room_id, _event_id)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "PUT", &uri, Some(&body)).await?;

    crate::xmatrix::verify_pdu_signatures(&state, &body, {
        let claims_origin = body["origin"].as_str().unwrap_or("");
        claims_origin
    })
    .await
    .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;

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
    if membership != "leave" {
        return Err(crate::error::AppError::BadRequest(
            "expected membership: leave".into(),
        ));
    }

    let pdu_event_id = body["event_id"]
        .as_str()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing event_id".into()))?;
    let origin_server_ts = body["origin_server_ts"]
        .as_i64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    // PDU を保存してから退出処理
    let content = body["content"].clone();
    let auth_events = body.get("auth_events");
    let prev_events = body.get("prev_events");
    db::events::store_pdu(
        &state.pool,
        &db::events::PduMeta {
            event_id: pdu_event_id,
            room_id: &room_id,
            sender,
            event_type: "m.room.member",
            state_key: Some(sender),
            content: &content,
            auth_events,
            prev_events,
            origin_server_ts,
        },
    )
    .await
    .map_err(crate::error::AppError::Internal)?;

    db::rooms::leave(&state.pool, sender, &room_id)
        .await
        .map_err(crate::error::AppError::Internal)?;

    Ok(Json(serde_json::json!({})))
}
