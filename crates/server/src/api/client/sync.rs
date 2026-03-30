use crate::{error::ApiResult, filter::FilterDef, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/sync", get(sync))
}

#[derive(Deserialize)]
struct SyncQuery {
    since: Option<String>,
    /// long-polling タイムアウト（ミリ秒）。0 または未指定は即時返却。
    timeout: Option<u64>,
    filter: Option<String>,
}

async fn sync(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<SyncQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // フィルター定義を取得
    let filter_json: Option<serde_json::Value> = if let Some(ref f) = query.filter {
        if let Ok(id) = f.parse::<u64>() {
            db::filters::get(&state.pool, &user.user_id, id)
                .await
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            serde_json::from_str(f).ok()
        }
    } else {
        None
    };
    let filter = filter_json.as_ref().map(FilterDef::from_json);

    // since トークンを解析: "{stream_ordering}_{acked_to_device_id}_{since_ms}_{typing_version}" または旧形式
    let (since_stream, acked_to_device_id, account_data_since_ms, since_typing_version) =
        parse_since(query.since.as_deref());

    let timeline_limit = filter.as_ref().and_then(|f| f.timeline_limit).unwrap_or(50);
    let mut result = db::sync::sync(
        &state.pool,
        &user.user_id,
        since_stream.as_deref(),
        timeline_limit,
    )
    .await?;

    // long-polling: since あり・timeout > 0・新イベントなし の場合は待機してから再試行
    let timeout_ms = query.timeout.unwrap_or(0);
    if timeout_ms > 0 && since_stream.is_some() && !sync_has_new_events(&result) {
        let notify = state.event_notify.clone();
        let deadline = tokio::time::sleep(Duration::from_millis(timeout_ms.min(30_000)));
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                _ = notify.notified() => {
                    result = db::sync::sync(
                        &state.pool,
                        &user.user_id,
                        since_stream.as_deref(),
                        timeline_limit,
                    )
                    .await?;
                    // 新イベントがあるか、typing が変化しているか（since_typing_version より後）
                    let (_, new_typing_ver) = state.typing.get_changed_since(since_typing_version);
                    if sync_has_new_events(&result) || new_typing_ver > since_typing_version {
                        break;
                    }
                    // まだ変化がなければ引き続き待つ
                }
            }
        }
    }

    // sync 時にプレゼンス last_active_ts を更新（ベストエフォート）
    let _ = db::presence::set_active(&state.pool, &user.user_id).await;

    // ルームごとのタグ（account_data m.tag 用）
    let all_tags = db::room_tags::get_all_for_user(&state.pool, &user.user_id)
        .await
        .unwrap_or_default();
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

    // ユーザー全 account_data を取得（グローバル + ルーム固有）。since がある場合は差分のみ
    let all_account_data =
        db::account_data::get_for_sync(&state.pool, &user.user_id, account_data_since_ms)
            .await
            .unwrap_or_default();
    // グローバル account_data イベント
    let mut global_account_data_events: Vec<serde_json::Value> = all_account_data
        .iter()
        .filter(|(room_id, _, _)| room_id.is_empty())
        .map(|(_, event_type, content)| {
            serde_json::json!({
                "type": event_type,
                "content": serde_json::from_str::<serde_json::Value>(content).unwrap_or_default(),
            })
        })
        .collect();
    // ルーム固有 account_data をルーム別に整理
    let mut room_account_data: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for (room_id, event_type, content) in &all_account_data {
        if !room_id.is_empty() {
            room_account_data.entry(room_id.clone()).or_default().push(serde_json::json!({
                "type": event_type,
                "content": serde_json::from_str::<serde_json::Value>(content).unwrap_or_default(),
            }));
        }
    }

    // typing 差分: since_typing_version 以降に変化したルームのセットを計算
    let (typing_changed, current_typing_version) =
        state.typing.get_changed_since(since_typing_version);
    let typing_changed_rooms: HashSet<String> = typing_changed
        .into_iter()
        .map(|(room_id, _)| room_id)
        .collect();

    let mut presence_user_ids: HashSet<String> = HashSet::new();

    if let Some(join_map) = result
        .get_mut("rooms")
        .and_then(|r| r.get_mut("join"))
        .and_then(|j| j.as_object_mut())
    {
        // ルームフィルター（rooms / not_rooms）適用
        if let Some(ref f) = filter {
            join_map.retain(|room_id, _| f.include_room(room_id));
        }

        for (room_id, room_data) in join_map.iter_mut() {
            // timeline フィルター
            if let Some(ref f) = filter {
                if let Some(timeline) = room_data.get_mut("timeline") {
                    if let Some(events) = timeline.get_mut("events") {
                        if let Some(arr) = events.as_array_mut() {
                            FilterDef::apply_event_filter(
                                arr,
                                &f.timeline_types,
                                &f.timeline_not_types,
                            );
                        }
                    }
                }
                // state フィルター
                if let Some(state_obj) = room_data.get_mut("state") {
                    if let Some(events) = state_obj.get_mut("events") {
                        if let Some(arr) = events.as_array_mut() {
                            FilterDef::apply_event_filter(arr, &f.state_types, &f.state_not_types);
                        }
                    }
                }

                // lazy_load_members: timeline に現れた sender の m.room.member のみ残す
                if f.lazy_load_members {
                    let timeline_senders: std::collections::HashSet<String> = room_data
                        .get("timeline")
                        .and_then(|t| t.get("events"))
                        .and_then(|e| e.as_array())
                        .map(|events| {
                            events
                                .iter()
                                .filter_map(|e| {
                                    e.get("sender")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    if let Some(state_obj) = room_data.get_mut("state") {
                        if let Some(events) = state_obj.get_mut("events") {
                            if let Some(arr) = events.as_array_mut() {
                                arr.retain(|e| {
                                    if e.get("type").and_then(|v| v.as_str())
                                        == Some("m.room.member")
                                    {
                                        let sk = e
                                            .get("state_key")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        timeline_senders.contains(sk)
                                    } else {
                                        true
                                    }
                                });
                            }
                        }
                    }
                }
            }

            // ephemeral イベント（m.typing / m.receipt）
            let mut ephemeral_events: Vec<serde_json::Value> = Vec::new();

            // typing 差分配信: since_typing_version より後に変化したルームのみ送信
            // 初回 sync（since なし）は全ルームの typing を送信する
            let include_typing = if since_typing_version > 0 {
                typing_changed_rooms.contains(room_id)
            } else {
                true
            };
            if include_typing {
                let typing_users = state.typing.get_typing(room_id);
                ephemeral_events.push(serde_json::json!({
                    "type": "m.typing",
                    "content": { "user_ids": typing_users },
                }));
            }

            if let Ok(receipts) = db::receipts::get_for_room(&state.pool, room_id).await {
                if !receipts.is_empty() {
                    ephemeral_events.push(db::receipts::to_event(receipts));
                }
            }

            // ephemeral フィルター
            if let Some(ref f) = filter {
                FilterDef::apply_event_filter(
                    &mut ephemeral_events,
                    &f.ephemeral_types,
                    &f.ephemeral_not_types,
                );
            }

            if let Some(ephemeral) = room_data.get_mut("ephemeral") {
                if let Some(events) = ephemeral.get_mut("events") {
                    *events = serde_json::json!(ephemeral_events);
                }
            }

            // account_data: m.tag + ルーム固有 account_data
            let mut account_data_events: Vec<serde_json::Value> = Vec::new();
            if let Some(tags) = tags_by_room.get(room_id) {
                if !tags.is_empty() {
                    account_data_events.push(serde_json::json!({
                        "type": "m.tag",
                        "content": { "tags": tags },
                    }));
                }
            }
            if let Some(room_events) = room_account_data.get(room_id) {
                account_data_events.extend_from_slice(room_events);
            }

            // account_data フィルター
            if let Some(ref f) = filter {
                FilterDef::apply_event_filter(
                    &mut account_data_events,
                    &f.account_data_types,
                    &f.account_data_not_types,
                );
            }

            if let Some(obj) = room_data.as_object_mut() {
                obj.insert(
                    "account_data".to_string(),
                    serde_json::json!({ "events": account_data_events }),
                );
            }

            // プレゼンス収集: ルームメンバーの user_id セットを集める
            if let Ok(statuses) = db::presence::get_for_room_members(&state.pool, room_id).await {
                for s in statuses {
                    presence_user_ids.insert(s.user_id);
                }
            }
        }
    }

    // presence.events — since_ms がある場合は差分のみ、初回 sync は全員分
    let presence_statuses: Vec<db::presence::PresenceStatus> =
        if let Some(since_ms) = account_data_since_ms {
            let uids: Vec<String> = presence_user_ids.into_iter().collect();
            db::presence::get_changed_since(&state.pool, &uids, since_ms as i64)
                .await
                .unwrap_or_default()
        } else {
            // 初回 sync: 全ユーザー分を個別取得
            let mut statuses = Vec::new();
            for uid in &presence_user_ids {
                if let Ok(Some(s)) = db::presence::get(&state.pool, uid).await {
                    statuses.push(s);
                }
            }
            statuses
        };

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut presence_events: Vec<serde_json::Value> = presence_statuses
        .into_iter()
        .map(|s| {
            let last_active_ago = now_ms - s.last_active_ts;
            let mut content = serde_json::json!({
                "presence": s.presence,
                "last_active_ago": last_active_ago,
                "currently_active": s.presence == "online" && last_active_ago < 60_000,
            });
            if let Some(msg) = &s.status_msg {
                content["status_msg"] = serde_json::json!(msg);
            }
            serde_json::json!({
                "type": "m.presence",
                "sender": s.user_id,
                "content": content,
            })
        })
        .collect();

    // presence フィルター
    if let Some(ref f) = filter {
        FilterDef::apply_event_filter(
            &mut presence_events,
            &f.presence_types,
            &f.presence_not_types,
        );
    }

    if let Some(presence) = result.get_mut("presence") {
        if let Some(events) = presence.get_mut("events") {
            *events = serde_json::json!(presence_events);
        }
    }

    // グローバル account_data フィルター
    if let Some(ref f) = filter {
        FilterDef::apply_event_filter(
            &mut global_account_data_events,
            &f.account_data_types,
            &f.account_data_not_types,
        );
    }
    result["account_data"] = serde_json::json!({ "events": global_account_data_events });

    // to_device.events（at-least-once 配信）
    // 前回 sync で返したメッセージを ack（since があれば acked_to_device_id 以下を削除）
    let _ = db::to_device::delete_acked(&state.pool, &user.user_id, acked_to_device_id).await;

    let pending = db::to_device::get_pending(&state.pool, &user.user_id, &user.device_id)
        .await
        .unwrap_or_default();
    let max_to_device_id = pending.iter().map(|m| m.id).max().unwrap_or(0);
    let to_device_events: Vec<serde_json::Value> = pending
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "type": m.event_type,
                "sender": m.sender,
                "content": serde_json::from_str::<serde_json::Value>(&m.content)
                    .unwrap_or_default(),
            })
        })
        .collect();

    result["to_device"] = serde_json::json!({ "events": to_device_events });

    // device_lists（E2EE デバイスキー変更通知）
    // since_stream を u64 に変換して changed / left を取得する
    let since_stream_ord: Option<u64> = since_stream.as_deref().and_then(|s| s.parse::<u64>().ok());
    let device_lists_changed =
        db::keys::get_changed_users(&state.pool, &user.user_id, account_data_since_ms)
            .await
            .unwrap_or_default();
    let device_lists_left = if let Some(ord) = since_stream_ord {
        db::keys::get_left_users(&state.pool, &user.user_id, ord)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };
    result["device_lists"] = serde_json::json!({
        "changed": device_lists_changed,
        "left": device_lists_left,
    });

    // next_batch を "{stream_ordering}_{max_to_device_id}_{now_ms}_{typing_version}" に更新
    let stream_ordering = result["next_batch"].as_str().unwrap_or("0").to_string();
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    result["next_batch"] = serde_json::json!(format!(
        "{stream_ordering}_{max_to_device_id}_{now_ms}_{current_typing_version}"
    ));

    Ok(Json(result))
}

/// since トークンを解析して (stream_ordering, acked_to_device_id, account_data_since_ms, typing_version) を返す
/// フォーマット: "{ord}_{to_device_id}_{since_ms}_{typing_ver}" / 旧形式も後方互換
fn parse_since(since: Option<&str>) -> (Option<String>, u64, Option<u64>, u64) {
    let Some(s) = since else {
        return (None, 0, None, 0);
    };
    let parts: Vec<&str> = s.splitn(4, '_').collect();
    let ord = Some(parts[0].to_string());
    let acked = parts
        .get(1)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let since_ms = parts.get(2).and_then(|v| v.parse::<u64>().ok());
    let typing_ver = parts
        .get(3)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    (ord, acked, since_ms, typing_ver)
}

/// 増分 sync 結果に新しいイベント（join timeline / invite / leave / to_device）があるか判定する。
/// long-polling の「起床後に新イベントがあるか」チェックに使う。
fn sync_has_new_events(result: &serde_json::Value) -> bool {
    // rooms.join に timeline イベントがあるか
    if let Some(join) = result["rooms"]["join"].as_object() {
        for room in join.values() {
            if let Some(events) = room["timeline"]["events"].as_array() {
                if !events.is_empty() {
                    return true;
                }
            }
        }
    }
    // rooms.invite / rooms.leave に何かあるか
    if let Some(invite) = result["rooms"]["invite"].as_object() {
        if !invite.is_empty() {
            return true;
        }
    }
    if let Some(leave) = result["rooms"]["leave"].as_object() {
        if !leave.is_empty() {
            return true;
        }
    }
    // to_device に何かあるか
    if let Some(events) = result["to_device"]["events"].as_array() {
        if !events.is_empty() {
            return true;
        }
    }
    false
}
