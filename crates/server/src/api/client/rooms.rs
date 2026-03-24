use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/createRoom", post(create_room))
        .route("/_matrix/client/v3/join/{roomIdOrAlias}", post(join_room))
        .route("/_matrix/client/v3/rooms/{roomId}/leave", post(leave_room))
        .route("/_matrix/client/v3/joined_rooms", get(joined_rooms))
}

#[derive(Deserialize)]
struct CreateRoomRequest {
    name: Option<String>,
    topic: Option<String>,
    preset: Option<String>,
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

    Ok(Json(CreateRoomResponse { room_id }))
}

/// ローカルルームへ状態イベントを保存する共通ヘルパー。
/// event_id を SHA-256 ハッシュで計算し、events・room_state テーブルに保存する。
async fn store_state_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: &str,
    content: &serde_json::Value,
) -> anyhow::Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let pdu_for_hash = serde_json::json!({
        "room_id": room_id,
        "sender": sender,
        "type": event_type,
        "state_key": state_key,
        "content": content,
        "origin_server_ts": now_ms,
        "origin": &*state.server_name,
        "depth": 0,
        "auth_events": [],
        "prev_events": [],
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
        },
    )
    .await?;
    Ok(())
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
    if let Ok(()) = store_state_event(
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
        let now_ms = chrono::Utc::now().timestamp_millis();
        let pdu_for_hash = serde_json::json!({
            "room_id": room_id,
            "sender": user.user_id,
            "type": "m.room.member",
            "state_key": user.user_id,
            "content": join_content,
            "origin_server_ts": now_ms,
            "origin": &*state.server_name,
            "depth": 0,
            "auth_events": [],
            "prev_events": [],
        });
        let event_id = crate::signing_key::compute_event_id(&pdu_for_hash);
        let mut pdu = pdu_for_hash;
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
    let now_ms = chrono::Utc::now().timestamp_millis();
    let pdu_for_hash = serde_json::json!({
        "room_id": room_id,
        "sender": user.user_id,
        "type": "m.room.member",
        "state_key": user.user_id,
        "content": leave_content,
        "origin_server_ts": now_ms,
        "origin": &*state.server_name,
        "depth": 0,
        "auth_events": [],
        "prev_events": [],
    });
    let event_id = crate::signing_key::compute_event_id(&pdu_for_hash);
    let _ = db::events::send(
        &state.pool,
        &db::events::LocalEvent {
            event_id: &event_id,
            room_id: &room_id,
            sender: &user.user_id,
            event_type: "m.room.member",
            state_key: Some(&user.user_id),
            content: &leave_content,
            origin_server_ts: now_ms,
        },
    )
    .await;

    db::rooms::leave(&state.pool, &user.user_id, &room_id).await?;

    // leave PDU を外部サーバーに配送（ベストエフォート）
    let mut pdu = pdu_for_hash;
    pdu["event_id"] = serde_json::Value::String(event_id);
    crate::federation_client::dispatch_send_transaction(state, room_id, pdu);

    Ok(Json(serde_json::json!({})))
}

async fn joined_rooms(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let rooms = db::rooms::joined_rooms(&state.pool, &user.user_id).await?;
    Ok(Json(serde_json::json!({ "joined_rooms": rooms })))
}
