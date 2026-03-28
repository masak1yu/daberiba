use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/createRoom", post(create_room))
        .route("/_matrix/client/v3/join/{roomIdOrAlias}", post(join_room))
        .route("/_matrix/client/v3/rooms/{roomId}/leave", post(leave_room))
        .route("/_matrix/client/v3/joined_rooms", get(joined_rooms))
        .route(
            "/_matrix/client/v3/rooms/{roomId}/redact/{eventId}/{txnId}",
            put(redact_event),
        )
        .route("/_matrix/client/v3/rooms/{roomId}/kick", post(kick_user))
        .route("/_matrix/client/v3/rooms/{roomId}/ban", post(ban_user))
        .route("/_matrix/client/v3/rooms/{roomId}/unban", post(unban_user))
        .route(
            "/_matrix/client/v3/rooms/{roomId}/forget",
            post(forget_room),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/upgrade",
            post(upgrade_room),
        )
        .route("/_matrix/client/v3/rooms/{roomId}/knock", post(knock_room))
        .route(
            "/_matrix/client/v3/knock/{roomIdOrAlias}",
            post(knock_room_alias),
        )
}

#[derive(Deserialize)]
struct CreateRoomRequest {
    name: Option<String>,
    topic: Option<String>,
    preset: Option<String>,
    room_alias_name: Option<String>,
}

#[derive(Serialize)]
struct CreateRoomResponse {
    room_id: String,
}

async fn create_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<CreateRoomRequest>,
) -> ApiResult<Json<CreateRoomResponse>> {
    let room_id = db::rooms::create(
        &state.pool,
        &user.user_id,
        body.name.as_deref(),
        body.topic.as_deref(),
        &state.server_name,
    )
    .await?;

    // ルーム作成時の必須状態イベントを生成・保存する。
    // これらは federation make_join / send_join で auth_chain として返すために必要。

    // m.room.create
    store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.create",
        "",
        &serde_json::json!({
            "creator": user.user_id,
            "room_version": "10",
        }),
    )
    .await?;

    // m.room.join_rules（preset: "public_chat" の場合は public）
    let join_rule = match body.preset.as_deref() {
        Some("public_chat") => "public",
        _ => "invite",
    };
    store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.join_rules",
        "",
        &serde_json::json!({ "join_rule": join_rule }),
    )
    .await?;

    // m.room.power_levels
    store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.power_levels",
        "",
        &serde_json::json!({
            "users": { &user.user_id: 100 },
            "users_default": 0,
            "events_default": 0,
            "state_default": 50,
            "ban": 50,
            "kick": 50,
            "redact": 50,
            "invite": 50,
        }),
    )
    .await?;

    // m.room.member — creator の join
    store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.member",
        &user.user_id,
        &serde_json::json!({ "membership": "join" }),
    )
    .await?;

    // m.room.name（指定時のみ）
    if let Some(name) = body.name.as_deref() {
        store_state_event(
            &state,
            &room_id,
            &user.user_id,
            "m.room.name",
            "",
            &serde_json::json!({ "name": name }),
        )
        .await?;
    }

    // m.room.topic（指定時のみ）
    if let Some(topic) = body.topic.as_deref() {
        store_state_event(
            &state,
            &room_id,
            &user.user_id,
            "m.room.topic",
            "",
            &serde_json::json!({ "topic": topic }),
        )
        .await?;
    }

    // room_alias_name が指定された場合: エイリアス登録 + m.room.canonical_alias 保存
    if let Some(alias_name) = body.room_alias_name.as_deref() {
        let alias = format!("#{}:{}", alias_name, &*state.server_name);
        // 登録失敗（重複など）はルーム作成失敗として扱う
        db::room_aliases::create(&state.pool, &alias, &room_id, &user.user_id).await?;
        store_state_event(
            &state,
            &room_id,
            &user.user_id,
            "m.room.canonical_alias",
            "",
            &serde_json::json!({ "alias": alias }),
        )
        .await?;
    }

    Ok(Json(CreateRoomResponse { room_id }))
}

/// ローカルルームへ状態イベントを保存する共通ヘルパー。
/// depth・prev_events・auth_events を DB から取得し、PDU ハッシュを計算して保存する。
/// 戻り値: (event_id, pdu)  — federation 配送に使用できる。
async fn store_state_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: &str,
    content: &serde_json::Value,
) -> anyhow::Result<(String, serde_json::Value)> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (tip_result, auth_result) = tokio::join!(
        db::events::get_room_tip(&state.pool, room_id),
        db::room_state::get_auth_event_ids(&state.pool, room_id),
    );
    let (depth, prev_event_ids) = tip_result?;
    let auth_event_ids = auth_result.unwrap_or_default();

    let pdu_for_hash = serde_json::json!({
        "room_id": room_id,
        "sender": sender,
        "type": event_type,
        "state_key": state_key,
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
            room_id,
            sender,
            event_type,
            state_key: Some(state_key),
            content,
            origin_server_ts: now_ms,
            depth,
            prev_events: &prev_event_ids,
        },
    )
    .await?;
    // /sync long-polling を起床させる
    state.event_notify.notify_waiters();
    Ok((event_id, pdu_for_hash))
}

async fn join_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id_or_alias): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // エイリアス（#で始まる）の場合はルームIDに解決する
    let room_id = if room_id_or_alias.starts_with('#') {
        db::room_aliases::resolve(&state.pool, &room_id_or_alias)
            .await?
            .ok_or(AppError::NotFound)?
    } else {
        room_id_or_alias.clone()
    };

    // 外部ルームの場合は federation join フロー（make_join → send_join）を実行する
    if !crate::federation_client::is_local_room(&state, &room_id) {
        crate::federation_client::join_remote_room(&state, &room_id, &user.user_id)
            .await
            .map_err(|e| {
                tracing::warn!(room_id, error = %e, "federation join 失敗");
                AppError::BadRequest(format!("federation join failed: {e}"))
            })?;
        return Ok(Json(serde_json::json!({ "room_id": room_id })));
    }

    db::rooms::join(&state.pool, &user.user_id, &room_id).await?;

    // join イベントを保存して外部サーバーへ配送
    let join_content = serde_json::json!({ "membership": "join" });
    if let Ok((event_id, mut pdu)) = store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.member",
        &user.user_id,
        &join_content,
    )
    .await
    {
        // join PDU を外部サーバーに配送（ベストエフォート）
        pdu["event_id"] = serde_json::Value::String(event_id);
        crate::federation_client::dispatch_send_transaction(state, room_id.clone(), pdu);
    }

    Ok(Json(serde_json::json!({ "room_id": room_id })))
}

async fn leave_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // 外部ルームの場合は federation leave フロー（make_leave → send_leave）を実行する
    if !crate::federation_client::is_local_room(&state, &room_id) {
        crate::federation_client::leave_remote_room(&state, &room_id, &user.user_id)
            .await
            .map_err(|e| {
                tracing::warn!(room_id, error = %e, "federation leave 失敗");
                AppError::BadRequest(format!("federation leave failed: {e}"))
            })?;
        return Ok(Json(serde_json::json!({})));
    }

    // leave イベントを保存してメンバーシップを更新
    let leave_content = serde_json::json!({ "membership": "leave" });
    let pdu_result = store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.member",
        &user.user_id,
        &leave_content,
    )
    .await;

    db::rooms::leave(&state.pool, &user.user_id, &room_id).await?;

    // leave PDU を外部サーバーに配送（ベストエフォート）
    if let Ok((event_id, mut pdu)) = pdu_result {
        pdu["event_id"] = serde_json::Value::String(event_id);
        crate::federation_client::dispatch_send_transaction(state, room_id, pdu);
    }

    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct KnockRequest {
    reason: Option<String>,
    #[serde(rename = "via")]
    #[allow(dead_code)]
    via: Option<Vec<String>>,
}

/// POST /_matrix/client/v3/rooms/{roomId}/knock — ノック（入室申請）
async fn knock_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    body: Option<Json<KnockRequest>>,
) -> ApiResult<Json<serde_json::Value>> {
    knock_room_impl(
        &state,
        &user.user_id,
        &room_id,
        body.and_then(|b| b.reason.clone()),
    )
    .await
}

/// POST /_matrix/client/v3/knock/{roomIdOrAlias}
async fn knock_room_alias(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id_or_alias): Path<String>,
    body: Option<Json<KnockRequest>>,
) -> ApiResult<Json<serde_json::Value>> {
    // エイリアスを解決
    let room_id = if room_id_or_alias.starts_with('#') {
        db::room_aliases::resolve(&state.pool, &room_id_or_alias)
            .await?
            .ok_or(AppError::NotFound)?
    } else {
        room_id_or_alias
    };
    knock_room_impl(
        &state,
        &user.user_id,
        &room_id,
        body.and_then(|b| b.reason.clone()),
    )
    .await
}

async fn knock_room_impl(
    state: &AppState,
    user_id: &str,
    room_id: &str,
    reason: Option<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // join_rules が knock または knock_restricted であることを確認
    let join_rules = db::room_state::get_event(&state.pool, room_id, "m.room.join_rules", "")
        .await?
        .and_then(|v| v["join_rule"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "invite".to_string());

    if !matches!(join_rules.as_str(), "knock" | "knock_restricted") {
        return Err(AppError::Forbidden);
    }

    // 現在の membership をチェック（ban されている場合はブロック）
    if let Some(membership) = db::rooms::get_membership(&state.pool, room_id, user_id).await? {
        if membership == "ban" {
            return Err(AppError::Forbidden);
        }
        if membership == "join" {
            return Err(AppError::BadRequest("already joined".into()));
        }
    }

    let mut content = serde_json::json!({ "membership": "knock" });
    if let Some(r) = reason {
        content["reason"] = serde_json::Value::String(r);
    }

    store_state_event(state, room_id, user_id, "m.room.member", user_id, &content).await?;
    db::rooms::knock(&state.pool, user_id, room_id).await?;

    Ok(Json(serde_json::json!({ "room_id": room_id })))
}

async fn joined_rooms(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let rooms = db::rooms::joined_rooms(&state.pool, &user.user_id).await?;
    Ok(Json(serde_json::json!({ "joined_rooms": rooms })))
}

#[derive(Deserialize)]
struct RedactRequest {
    reason: Option<String>,
}

/// PUT /_matrix/client/v3/rooms/{roomId}/redact/{eventId}/{txnId}
async fn redact_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((room_id, target_event_id, _txn_id)): Path<(String, String, String)>,
    Json(body): Json<RedactRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // 自分のイベントでなければ redact パワーレベルが必要
    let target_event = db::events::get_by_id(&state.pool, &target_event_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let target_sender = target_event
        .get("sender")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if target_sender != user.user_id {
        let (caller_pl, required_pl) = tokio::join!(
            db::room_state::get_user_power_level(&state.pool, &room_id, &user.user_id),
            db::room_state::get_required_power_level(&state.pool, &room_id, "redact"),
        );
        if caller_pl? < required_pl? {
            return Err(AppError::Forbidden);
        }
    }

    let mut content = serde_json::json!({ "redacts": target_event_id });
    if let Some(reason) = body.reason {
        content["reason"] = serde_json::Value::String(reason);
    }
    let (redaction_event_id, _pdu) = store_message_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.redaction",
        &content,
    )
    .await?;
    db::events::redact_event(&state.pool, &target_event_id).await?;
    Ok(Json(serde_json::json!({ "event_id": redaction_event_id })))
}

#[derive(Deserialize)]
struct ModerationRequest {
    user_id: String,
    reason: Option<String>,
}

/// POST /_matrix/client/v3/rooms/{roomId}/kick
async fn kick_user(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<ModerationRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // パワーレベルチェック
    let (caller_pl, required_pl) = tokio::join!(
        db::room_state::get_user_power_level(&state.pool, &room_id, &user.user_id),
        db::room_state::get_required_power_level(&state.pool, &room_id, "kick"),
    );
    if caller_pl? < required_pl? {
        return Err(AppError::Forbidden);
    }

    let mut content = serde_json::json!({ "membership": "leave" });
    if let Some(reason) = body.reason {
        content["reason"] = serde_json::Value::String(reason);
    }
    store_state_event(
        &state,
        &room_id,
        &body.user_id,
        "m.room.member",
        &body.user_id,
        &content,
    )
    .await?;
    db::rooms::leave(&state.pool, &body.user_id, &room_id).await?;
    Ok(Json(serde_json::json!({})))
}

/// POST /_matrix/client/v3/rooms/{roomId}/ban
async fn ban_user(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<ModerationRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // パワーレベルチェック
    let (caller_pl, required_pl) = tokio::join!(
        db::room_state::get_user_power_level(&state.pool, &room_id, &user.user_id),
        db::room_state::get_required_power_level(&state.pool, &room_id, "ban"),
    );
    if caller_pl? < required_pl? {
        return Err(AppError::Forbidden);
    }

    let mut content = serde_json::json!({ "membership": "ban" });
    if let Some(reason) = body.reason {
        content["reason"] = serde_json::Value::String(reason);
    }
    store_state_event(
        &state,
        &room_id,
        &body.user_id,
        "m.room.member",
        &body.user_id,
        &content,
    )
    .await?;
    db::rooms::ban(&state.pool, &room_id, &body.user_id).await?;
    Ok(Json(serde_json::json!({})))
}

/// POST /_matrix/client/v3/rooms/{roomId}/unban
async fn unban_user(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<ModerationRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // パワーレベルチェック（ban 権限が必要）
    let (caller_pl, required_pl) = tokio::join!(
        db::room_state::get_user_power_level(&state.pool, &room_id, &user.user_id),
        db::room_state::get_required_power_level(&state.pool, &room_id, "ban"),
    );
    if caller_pl? < required_pl? {
        return Err(AppError::Forbidden);
    }

    db::rooms::unban(&state.pool, &room_id, &body.user_id).await?;
    Ok(Json(serde_json::json!({})))
}

/// POST /_matrix/client/v3/rooms/{roomId}/forget
async fn forget_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    db::rooms::forget(&state.pool, &room_id, &user.user_id).await?;
    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct UpgradeRoomRequest {
    new_version: String,
}

/// POST /_matrix/client/v3/rooms/{roomId}/upgrade
/// ルームバージョンをアップグレードする。
/// 旧ルームを tombstone 状態にし、新ルームを作成して m.room.create に predecessor を設定する。
async fn upgrade_room(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<UpgradeRoomRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // パワーレベルチェック（state_default = 50 が必要）
    let (caller_pl, state_default) = tokio::join!(
        db::room_state::get_user_power_level(&state.pool, &room_id, &user.user_id),
        db::room_state::get_required_power_level(&state.pool, &room_id, "state_default"),
    );
    if caller_pl? < state_default? {
        return Err(AppError::Forbidden);
    }

    // 旧ルームの最終 event_id を取得（predecessor に設定するため）
    let (predecessor_depth, predecessor_event_ids) =
        db::events::get_room_tip(&state.pool, &room_id).await?;
    let predecessor_event_id = if predecessor_depth > 0 {
        predecessor_event_ids.first().cloned().unwrap_or_default()
    } else {
        String::new()
    };

    // 新ルームを作成
    let new_room_id =
        db::rooms::create(&state.pool, &user.user_id, None, None, &state.server_name).await?;

    // 新ルームのバージョンを設定
    db::rooms::set_version(&state.pool, &new_room_id, &body.new_version).await?;

    // 新ルームに必須状態イベントを生成（m.room.create に predecessor を含む）
    store_state_event(
        &state,
        &new_room_id,
        &user.user_id,
        "m.room.create",
        "",
        &serde_json::json!({
            "creator": user.user_id,
            "room_version": body.new_version,
            "predecessor": { "room_id": room_id, "event_id": predecessor_event_id },
        }),
    )
    .await?;

    store_state_event(
        &state,
        &new_room_id,
        &user.user_id,
        "m.room.join_rules",
        "",
        &serde_json::json!({ "join_rule": "invite" }),
    )
    .await?;

    store_state_event(
        &state,
        &new_room_id,
        &user.user_id,
        "m.room.power_levels",
        "",
        &serde_json::json!({
            "users": { &user.user_id: 100 },
            "users_default": 0,
            "events_default": 0,
            "state_default": 50,
            "ban": 50,
            "kick": 50,
            "redact": 50,
            "invite": 50,
        }),
    )
    .await?;

    store_state_event(
        &state,
        &new_room_id,
        &user.user_id,
        "m.room.member",
        &user.user_id,
        &serde_json::json!({ "membership": "join" }),
    )
    .await?;

    // 旧ルームの name / topic / avatar を新ルームにコピー
    let (old_name, old_topic, old_avatar) = tokio::join!(
        db::room_state::get_event(&state.pool, &room_id, "m.room.name", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.topic", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.avatar", ""),
    );
    if let Some(content) = old_name? {
        store_state_event(
            &state,
            &new_room_id,
            &user.user_id,
            "m.room.name",
            "",
            &content,
        )
        .await?;
    }
    if let Some(content) = old_topic? {
        store_state_event(
            &state,
            &new_room_id,
            &user.user_id,
            "m.room.topic",
            "",
            &content,
        )
        .await?;
    }
    if let Some(content) = old_avatar? {
        store_state_event(
            &state,
            &new_room_id,
            &user.user_id,
            "m.room.avatar",
            "",
            &content,
        )
        .await?;
    }

    // 旧ルームに m.room.tombstone を保存してアップグレード済みにする
    store_state_event(
        &state,
        &room_id,
        &user.user_id,
        "m.room.tombstone",
        "",
        &serde_json::json!({
            "body": "This room has been upgraded",
            "replacement_room": new_room_id,
        }),
    )
    .await?;

    Ok(Json(serde_json::json!({ "replacement_room": new_room_id })))
}

/// メッセージイベント（state_key なし）を保存する共通ヘルパー。
/// 戻り値: (event_id, pdu)
async fn store_message_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    content: &serde_json::Value,
) -> anyhow::Result<(String, serde_json::Value)> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (tip_result, auth_result) = tokio::join!(
        db::events::get_room_tip(&state.pool, room_id),
        db::room_state::get_auth_event_ids(&state.pool, room_id),
    );
    let (depth, prev_event_ids) = tip_result?;
    let auth_event_ids = auth_result.unwrap_or_default();

    let pdu_for_hash = serde_json::json!({
        "room_id": room_id,
        "sender": sender,
        "type": event_type,
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
            room_id,
            sender,
            event_type,
            state_key: None,
            content,
            origin_server_ts: now_ms,
            depth,
            prev_events: &prev_event_ids,
        },
    )
    .await?;
    Ok((event_id, pdu_for_hash))
}
