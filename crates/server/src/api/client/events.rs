use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}",
            put(send_event),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/state/{eventType}",
            put(send_state_event),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}",
            put(send_state_event_with_key),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/messages",
            get(get_messages),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/context/{eventId}",
            get(get_context),
        )
}

#[derive(Deserialize)]
struct SendEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "txnId")]
    _txn_id: String,
}

async fn send_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<SendEventPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (tip_result, auth_result) = tokio::join!(
        db::events::get_room_tip(&state.pool, &path.room_id),
        db::room_state::get_auth_event_ids(&state.pool, &path.room_id),
    );
    let (depth, prev_event_ids) = tip_result?;
    let auth_event_ids = auth_result.unwrap_or_default();

    // PDU を組み立てて event_id（room v3+ SHA-256 ハッシュ）を計算する
    let pdu_for_hash = serde_json::json!({
        "room_id": path.room_id,
        "sender": user.user_id,
        "type": path.event_type,
        "content": content,
        "origin_server_ts": now_ms,
        "origin": &*state.server_name,
        "depth": depth,
        "auth_events": auth_event_ids,
        "prev_events": prev_event_ids,
    });
    let event_id = crate::signing_key::compute_event_id(&pdu_for_hash);

    db::events::send(
        &state.pool,
        &db::events::LocalEvent {
            event_id: &event_id,
            room_id: &path.room_id,
            sender: &user.user_id,
            event_type: &path.event_type,
            state_key: None,
            content: &content,
            origin_server_ts: now_ms,
            depth,
            prev_events: &prev_event_ids,
        },
    )
    .await?;

    // federation 配送（背景タスク、ベストエフォート）
    let mut pdu = pdu_for_hash;
    pdu["event_id"] = serde_json::Value::String(event_id.clone());
    crate::federation_client::dispatch_send_transaction(state.clone(), path.room_id.clone(), pdu);

    // HTTP pusher への通知（背景タスク、ベストエフォート）
    dispatch_push(
        state,
        path.room_id,
        event_id.clone(),
        user.user_id,
        path.event_type,
        content,
    );

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

#[derive(Deserialize)]
struct StateEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
}

async fn send_state_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (tip_result, auth_result) = tokio::join!(
        db::events::get_room_tip(&state.pool, &path.room_id),
        db::room_state::get_auth_event_ids(&state.pool, &path.room_id),
    );
    let (depth, prev_event_ids) = tip_result?;
    let auth_event_ids = auth_result.unwrap_or_default();

    let pdu_for_hash = serde_json::json!({
        "room_id": path.room_id,
        "sender": user.user_id,
        "type": path.event_type,
        "state_key": "",
        "content": content,
        "origin_server_ts": now_ms,
        "origin": &*state.server_name,
        "depth": depth,
        "auth_events": auth_event_ids,
        "prev_events": prev_event_ids,
    });
    let event_id = crate::signing_key::compute_event_id(&pdu_for_hash);

    db::events::send(
        &state.pool,
        &db::events::LocalEvent {
            event_id: &event_id,
            room_id: &path.room_id,
            sender: &user.user_id,
            event_type: &path.event_type,
            state_key: Some(""),
            content: &content,
            origin_server_ts: now_ms,
            depth,
            prev_events: &prev_event_ids,
        },
    )
    .await?;

    let mut pdu = pdu_for_hash;
    pdu["event_id"] = serde_json::Value::String(event_id.clone());
    crate::federation_client::dispatch_send_transaction(state, path.room_id.clone(), pdu);

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

#[derive(Deserialize)]
struct StateEventWithKeyPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "stateKey")]
    state_key: String,
}

async fn send_state_event_with_key(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventWithKeyPath>,
    Json(content): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (tip_result, auth_result) = tokio::join!(
        db::events::get_room_tip(&state.pool, &path.room_id),
        db::room_state::get_auth_event_ids(&state.pool, &path.room_id),
    );
    let (depth, prev_event_ids) = tip_result?;
    let auth_event_ids = auth_result.unwrap_or_default();

    let pdu_for_hash = serde_json::json!({
        "room_id": path.room_id,
        "sender": user.user_id,
        "type": path.event_type,
        "state_key": path.state_key,
        "content": content,
        "origin_server_ts": now_ms,
        "origin": &*state.server_name,
        "depth": depth,
        "auth_events": auth_event_ids,
        "prev_events": prev_event_ids,
    });
    let event_id = crate::signing_key::compute_event_id(&pdu_for_hash);

    db::events::send(
        &state.pool,
        &db::events::LocalEvent {
            event_id: &event_id,
            room_id: &path.room_id,
            sender: &user.user_id,
            event_type: &path.event_type,
            state_key: Some(&path.state_key),
            content: &content,
            origin_server_ts: now_ms,
            depth,
            prev_events: &prev_event_ids,
        },
    )
    .await?;

    let mut pdu = pdu_for_hash;
    pdu["event_id"] = serde_json::Value::String(event_id.clone());
    crate::federation_client::dispatch_send_transaction(state, path.room_id.clone(), pdu);

    Ok(Json(serde_json::json!({ "event_id": event_id })))
}

#[derive(Deserialize)]
struct MessagesQuery {
    /// ページネーショントークン（"s{stream_ordering}"）。未指定時は先端から取得。
    from: Option<String>,
    /// 方向: "b"（新しい順、デフォルト）または "f"（古い順）
    dir: Option<String>,
    /// 取得件数（デフォルト 10、最大 100）
    limit: Option<u32>,
}

#[derive(Serialize)]
struct MessagesResponse {
    chunk: Vec<serde_json::Value>,
    start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
}

/// room_id のメンバー（sender 除く）の HTTP pusher に通知を送る。
/// push rule 評価を行い、notify アクションがある場合のみ配送する。
/// tokio::spawn で非同期に実行し、エラーはログのみ。
fn dispatch_push(
    state: AppState,
    room_id: String,
    event_id: String,
    sender: String,
    event_type: String,
    content: serde_json::Value,
) {
    tokio::spawn(async move {
        let pushers = match db::pushers::get_for_room_members(&state.pool, &room_id, &sender).await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "failed to fetch pushers");
                return;
            }
        };

        if pushers.is_empty() {
            return;
        }

        // ルームメンバー数を取得（room_member_count 条件用）
        let member_count = db::rooms::count_joined_members(&state.pool, &room_id)
            .await
            .unwrap_or(0);

        // push rule 評価用イベントオブジェクト
        let event_obj = serde_json::json!({
            "type": event_type,
            "sender": sender,
            "content": content,
            "room_id": room_id,
        });

        // pusher をユーザー単位にまとめる
        let mut by_user: std::collections::HashMap<String, Vec<db::pushers::Pusher>> =
            std::collections::HashMap::new();
        for p in pushers {
            by_user.entry(p.user_id.clone()).or_default().push(p);
        }

        for (user_id, user_pushers) in by_user {
            // ユーザーのプッシュルールをロード
            let rules = if let Ok(Some(v)) =
                db::account_data::get(&state.pool, &user_id, "", "m.push_rules").await
            {
                if v.get("global").map(|g| g.is_object()).unwrap_or(false) {
                    v
                } else {
                    default_push_rules_for(&user_id)
                }
            } else {
                default_push_rules_for(&user_id)
            };

            // 表示名を取得（contains_display_name 条件用）
            let display_name: Option<String> = db::profile::get(&state.pool, &user_id)
                .await
                .ok()
                .flatten()
                .and_then(|p| p["displayname"].as_str().map(|s| s.to_string()));

            // push rule 評価
            let actions = crate::push_eval::eval_push_rules(
                &rules,
                &event_obj,
                member_count,
                display_name.as_deref(),
            );
            let should_notify = actions
                .as_ref()
                .map(|a| crate::push_eval::actions_notify(a))
                .unwrap_or(false);

            if !should_notify {
                continue;
            }

            for pusher in user_pushers {
                if pusher.kind != "http" {
                    continue;
                }
                let data: serde_json::Value =
                    serde_json::from_str(&pusher.data).unwrap_or_default();
                let Some(url) = data.get("url").and_then(|v| v.as_str()) else {
                    continue;
                };
                let payload = serde_json::json!({
                    "notification": {
                        "event_id": event_id,
                        "room_id": room_id,
                        "type": event_type,
                        "sender": sender,
                        "content": content,
                        "devices": [{
                            "app_id": pusher.app_id,
                            "pushkey": pusher.pushkey,
                            "pushkey_ts": 0,
                            "data": data,
                        }],
                    }
                });
                if let Err(e) = state.http.post(url).json(&payload).send().await {
                    tracing::warn!(url, error = %e, "http push failed");
                }
            }
        }
    });
}

/// ユーザーのデフォルト push rules を生成する（account_data がない場合のフォールバック）
fn default_push_rules_for(user_id: &str) -> serde_json::Value {
    let localpart = user_id
        .split(':')
        .next()
        .unwrap_or(user_id)
        .trim_start_matches('@');
    serde_json::json!({
        "global": {
            "override": [
                {
                    "rule_id": ".m.rule.master",
                    "default": true,
                    "enabled": false,
                    "conditions": [],
                    "actions": ["dont_notify"]
                },
                {
                    "rule_id": ".m.rule.suppress_notices",
                    "default": true,
                    "enabled": true,
                    "conditions": [{"kind": "event_match", "key": "content.msgtype", "pattern": "m.notice"}],
                    "actions": ["dont_notify"]
                },
                {
                    "rule_id": ".m.rule.contains_display_name",
                    "default": true,
                    "enabled": true,
                    "conditions": [{"kind": "contains_display_name"}],
                    "actions": ["notify", {"set_tweak": "sound", "value": "default"}, {"set_tweak": "highlight"}]
                }
            ],
            "content": [
                {
                    "rule_id": ".m.rule.contains_user_name",
                    "default": true,
                    "enabled": true,
                    "pattern": localpart,
                    "actions": ["notify", {"set_tweak": "sound", "value": "default"}, {"set_tweak": "highlight"}]
                }
            ],
            "room": [],
            "sender": [],
            "underride": [
                {
                    "rule_id": ".m.rule.message",
                    "default": true,
                    "enabled": true,
                    "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.message"}],
                    "actions": ["notify"]
                },
                {
                    "rule_id": ".m.rule.encrypted",
                    "default": true,
                    "enabled": true,
                    "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.encrypted"}],
                    "actions": ["notify"]
                }
            ]
        }
    })
}

async fn get_messages(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(query): Query<MessagesQuery>,
) -> ApiResult<Json<MessagesResponse>> {
    let dir = query.dir.as_deref().unwrap_or("b");
    let limit = query.limit.unwrap_or(10).min(100);
    let from = query.from.as_deref().and_then(db::events::parse_token);

    let page = db::events::get_messages(&state.pool, &room_id, from, dir, limit).await?;

    Ok(Json(MessagesResponse {
        chunk: page.events,
        start: page.start,
        end: page.end,
    }))
}

#[derive(Deserialize)]
struct ContextQuery {
    limit: Option<u32>,
}

async fn get_context(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path((room_id, event_id)): Path<(String, String)>,
    Query(query): Query<ContextQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    use crate::error::AppError;

    let limit = query.limit.unwrap_or(10).min(100);
    let ctx = db::events::get_context(&state.pool, &room_id, &event_id, limit)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(serde_json::json!({
        "start": ctx.start,
        "end": ctx.end,
        "event": ctx.event,
        "events_before": ctx.events_before,
        "events_after": ctx.events_after,
        "state": [],
    })))
}
