use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/sync", get(sync))
}

#[derive(Deserialize)]
struct SyncQuery {
    since: Option<String>,
    #[allow(dead_code)]
    timeout: Option<u64>,
    filter: Option<String>,
}

async fn sync(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<SyncQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // filter_id が指定されている場合は DB からフィルター定義を取得
    let filter_def: Option<serde_json::Value> = if let Some(ref f) = query.filter {
        if let Ok(id) = f.parse::<u64>() {
            db::filters::get(&state.pool, &user.user_id, id)
                .await
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            // インライン JSON フィルター
            serde_json::from_str(f).ok()
        }
    } else {
        None
    };

    // タイムラインの許可イベントタイプ集合（フィルター未指定なら全許可）
    let allowed_timeline_types: Option<HashSet<String>> = filter_def
        .as_ref()
        .and_then(|f| f.get("room"))
        .and_then(|r| r.get("timeline"))
        .and_then(|t| t.get("types"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });

    let mut result = db::sync::sync(&state.pool, &user.user_id, query.since.as_deref()).await?;

    // ルームごとのタグを取得（account_data m.tag 用）
    let all_tags = db::room_tags::get_all_for_user(&state.pool, &user.user_id)
        .await
        .unwrap_or_default();

    // room_id -> タグマップ
    let mut tags_by_room: HashMap<String, serde_json::Map<String, serde_json::Value>> =
        HashMap::new();
    for (room_id, tag) in all_tags {
        let entry = tags_by_room.entry(room_id).or_default();
        let mut content = serde_json::Map::new();
        if let Some(order) = tag.order {
            content.insert("order".to_string(), serde_json::json!(order));
        }
        entry.insert(tag.tag, serde_json::Value::Object(content));
    }

    // 各参加ルームに ephemeral / account_data を付加、フィルター適用
    let mut presence_user_ids: HashSet<String> = HashSet::new();

    if let Some(join_map) = result
        .get_mut("rooms")
        .and_then(|r| r.get_mut("join"))
        .and_then(|j| j.as_object_mut())
    {
        for (room_id, room_data) in join_map.iter_mut() {
            // タイムラインフィルター適用
            if let Some(ref allowed) = allowed_timeline_types {
                if let Some(timeline) = room_data.get_mut("timeline") {
                    if let Some(events) = timeline.get_mut("events") {
                        if let Some(arr) = events.as_array_mut() {
                            arr.retain(|e| {
                                e.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| allowed.contains(t))
                                    .unwrap_or(true)
                            });
                        }
                    }
                }
            }

            // ephemeral イベント（m.typing / m.receipt）
            let mut ephemeral_events: Vec<serde_json::Value> = Vec::new();

            let typing_users = state.typing.get_typing(room_id);
            ephemeral_events.push(serde_json::json!({
                "type": "m.typing",
                "content": { "user_ids": typing_users },
            }));

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

            // account_data: m.tag
            let mut account_data_events: Vec<serde_json::Value> = Vec::new();
            if let Some(tags) = tags_by_room.get(room_id) {
                if !tags.is_empty() {
                    account_data_events.push(serde_json::json!({
                        "type": "m.tag",
                        "content": { "tags": tags },
                    }));
                }
            }
            if let Some(obj) = room_data.as_object_mut() {
                obj.insert(
                    "account_data".to_string(),
                    serde_json::json!({ "events": account_data_events }),
                );
            }

            // プレゼンス収集
            if let Ok(statuses) = db::presence::get_for_room_members(&state.pool, room_id).await {
                for s in statuses {
                    presence_user_ids.insert(s.user_id);
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
