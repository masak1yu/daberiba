/// Federation send transaction
/// PUT /_matrix/federation/v1/send/{txnId}
///
/// 他サーバーから PDU（イベント）を受け取り、自サーバーが参加しているルームに限り処理する。
/// レスポンスは処理結果を PDU ごとに返す。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::put,
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/federation/v1/send/:txn_id", put(send_transaction))
}

async fn send_transaction(
    State(state): State<AppState>,
    Path(_txn_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let claims = crate::xmatrix::verify_request(&state, &headers, "PUT", &uri, Some(&body)).await?;

    let pdus = body["pdus"]
        .as_array()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing pdus".into()))?;

    // ルームごとの参加確認結果をキャッシュして N+1 クエリを避ける
    let mut room_cache: HashMap<String, bool> = HashMap::new();

    let mut pdu_results = serde_json::Map::new();
    for pdu in pdus {
        let event_id = pdu["event_id"].as_str().unwrap_or("").to_string();
        match process_pdu(
            &state,
            pdu,
            &state.server_name,
            &claims.origin,
            &mut room_cache,
        )
        .await
        {
            Ok(()) => {
                pdu_results.insert(event_id, json!({}));
            }
            Err(e) => {
                tracing::warn!(event_id = %event_id, error = %e, "PDU 処理失敗");
                pdu_results.insert(event_id, json!({ "error": e.to_string() }));
            }
        }
    }

    // フェデレーションで受信したイベントの /sync long-polling を起床させる
    state.event_notify.notify_waiters();

    Ok(Json(json!({ "pdus": pdu_results })))
}

/// 単一 PDU を処理する。
/// 自サーバーが参加していないルームのイベントは無視する。
/// room_cache によりトランザクション内の同一ルームへの重複クエリを排除する。
async fn process_pdu(
    state: &AppState,
    pdu: &serde_json::Value,
    server_name: &str,
    origin: &str,
    room_cache: &mut HashMap<String, bool>,
) -> anyhow::Result<()> {
    // PDU 自体の Ed25519 署名を検証
    crate::xmatrix::verify_pdu_signatures(state, pdu, origin).await?;

    let room_id = pdu["room_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing room_id"))?;
    let sender = pdu["sender"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing sender"))?;
    let event_type = pdu["type"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing type"))?;
    let content = pdu.get("content").cloned().unwrap_or_default();
    let state_key: Option<&str> = pdu["state_key"].as_str();
    let origin_server_ts = pdu["origin_server_ts"]
        .as_i64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let pdu_event_id = pdu["event_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing event_id"))?
        .to_string();

    let we_are_in_room = match room_cache.get(room_id) {
        Some(&v) => v,
        None => {
            let members = db::rooms::get_joined_members(&state.pool, room_id)
                .await
                .unwrap_or_default();
            let in_room = members
                .keys()
                .any(|uid| uid.ends_with(&format!(":{server_name}")));
            room_cache.insert(room_id.to_string(), in_room);
            in_room
        }
    };
    if !we_are_in_room {
        return Ok(());
    }

    let auth_events = pdu.get("auth_events");
    let prev_events = pdu.get("prev_events");
    db::events::store_pdu(
        &state.pool,
        &db::events::PduMeta {
            event_id: &pdu_event_id,
            room_id,
            sender,
            event_type,
            state_key,
            content: &content,
            auth_events,
            prev_events,
            origin_server_ts,
            depth: pdu["depth"].as_i64().unwrap_or(0),
        },
    )
    .await?;

    if event_type == "m.room.member" {
        if let Some(sk) = state_key {
            let membership = content["membership"].as_str().unwrap_or("leave");
            match membership {
                "join" => {
                    db::rooms::join(&state.pool, sk, room_id).await?;
                }
                "leave" | "ban" => {
                    db::rooms::leave(&state.pool, sk, room_id).await?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
