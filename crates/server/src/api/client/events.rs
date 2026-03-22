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
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        None,
        &content,
    )
    .await?;

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
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        Some(""),
        &content,
    )
    .await?;

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
    let event_id = db::events::send(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.event_type,
        Some(&path.state_key),
        &content,
    )
    .await?;

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

        for pusher in pushers {
            if pusher.kind != "http" {
                continue;
            }
            let data: serde_json::Value = serde_json::from_str(&pusher.data).unwrap_or_default();
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
    });
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
