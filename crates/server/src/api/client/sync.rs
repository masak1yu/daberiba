use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashSet;

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
    // 同時にプレゼンス収集用のユーザー集合も構築
    let mut presence_user_ids: HashSet<String> = HashSet::new();

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

            // ルームメンバーのプレゼンス対象ユーザーを収集
            if let Ok(statuses) = db::presence::get_for_room_members(&state.pool, room_id).await {
                for s in statuses {
                    if presence_user_ids.insert(s.user_id.clone()) {
                        // 重複しないユーザーを presence イベントとして後で追加
                        let _ = s; // 後で再取得するため破棄
                    }
                }
            }
        }
    }

    // presence.events を構築
    let mut presence_events: Vec<serde_json::Value> = Vec::new();
    for uid in &presence_user_ids {
        if let Ok(Some(s)) = db::presence::get(&state.pool, uid).await {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let last_active_ago = now_ms - s.last_active_ts;
            let mut content = serde_json::json!({
                "presence": s.presence,
                "last_active_ago": last_active_ago,
                "currently_active": s.presence == "online" && last_active_ago < 60_000,
            });
            if let Some(msg) = &s.status_msg {
                content["status_msg"] = serde_json::json!(msg);
            }
            presence_events.push(serde_json::json!({
                "type": "m.presence",
                "sender": uid,
                "content": content,
            }));
        }
    }

    if let Some(presence) = result.get_mut("presence") {
        if let Some(events) = presence.get_mut("events") {
            *events = serde_json::json!(presence_events);
        }
    }

    Ok(Json(result))
}
