use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/rooms/:roomId/state", get(get_state))
        .route(
            "/_matrix/client/v3/rooms/:roomId/state/:eventType",
            get(get_state_event_no_key),
        )
        .route(
            "/_matrix/client/v3/rooms/:roomId/state/:eventType/:stateKey",
            get(get_state_event),
        )
        .route("/_matrix/client/v3/rooms/:roomId/members", get(get_members))
        .route(
            "/_matrix/client/v3/rooms/:roomId/joined_members",
            get(get_joined_members),
        )
        .route("/_matrix/client/v3/rooms/:roomId/invite", post(invite))
        .route(
            "/_matrix/client/v3/rooms/:roomId/report/:eventId",
            post(report_event),
        )
}

async fn get_state(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let events = db::room_state::get_all(&state.pool, &room_id).await?;
    Ok(Json(serde_json::json!(events)))
}

/// GET /_matrix/client/v3/rooms/:roomId/state/:eventType
/// state_key が空文字列のイベントコンテンツを返す（m.room.name, m.room.power_levels 等）。
async fn get_state_event_no_key(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path((room_id, event_type)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = db::room_state::get_event(&state.pool, &room_id, &event_type, "")
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(content))
}

#[derive(Deserialize)]
struct StateEventPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "stateKey")]
    state_key: String,
}

async fn get_state_event(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(path): Path<StateEventPath>,
) -> ApiResult<Json<serde_json::Value>> {
    let content = db::room_state::get_event(
        &state.pool,
        &path.room_id,
        &path.event_type,
        &path.state_key,
    )
    .await?
    .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(content))
}

#[derive(Deserialize, Default)]
struct MembersQuery {
    membership: Option<String>,
    not_membership: Option<String>,
    /// prev_batch / next_batch 形式のトークン。指定時は時点スナップショットを返す。
    at: Option<String>,
}

async fn get_members(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(query): Query<MembersQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let members = if let Some(at_token) = &query.at {
        // at トークンを stream_ordering に変換して時点スナップショットを返す。
        let at_ordering = db::events::parse_token(at_token).unwrap_or(u64::MAX);
        db::rooms::get_members_at(
            &state.pool,
            &room_id,
            at_ordering,
            query.membership.as_deref(),
            query.not_membership.as_deref(),
        )
        .await?
    } else {
        db::rooms::get_members_filtered(
            &state.pool,
            &room_id,
            query.membership.as_deref(),
            query.not_membership.as_deref(),
        )
        .await?
    };
    Ok(Json(serde_json::json!({ "chunk": members })))
}

async fn get_joined_members(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let members = db::rooms::get_joined_members(&state.pool, &room_id).await?;
    Ok(Json(serde_json::json!({ "joined": members })))
}

#[derive(Deserialize)]
struct InviteBody {
    user_id: String,
}

async fn invite(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<InviteBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::rooms::invite(&state.pool, &room_id, &user.user_id, &body.user_id).await?;

    // 被招待者の HTTP pusher に通知（ベストエフォート）
    let state2 = state.clone();
    let room_id2 = room_id.clone();
    let inviter = user.user_id.clone();
    let invitee = body.user_id.clone();
    tokio::spawn(async move {
        if let Ok(pushers) = db::pushers::list(&state2.pool, &invitee).await {
            for p in pushers {
                if p.kind != "http" {
                    continue;
                }
                let data: serde_json::Value = serde_json::from_str(&p.data).unwrap_or_default();
                let Some(url) = data.get("url").and_then(|v| v.as_str()) else {
                    continue;
                };
                let payload = serde_json::json!({
                    "notification": {
                        "room_id": room_id2,
                        "type": "m.room.member",
                        "sender": inviter,
                        "content": { "membership": "invite" },
                        "devices": [{ "app_id": p.app_id, "pushkey": p.pushkey }],
                    }
                });
                if let Err(e) = state2.http.post(url).json(&payload).send().await {
                    tracing::warn!(url, error = %e, "invite push failed");
                }
            }
        }
    });

    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct ReportBody {
    score: Option<i32>,
    reason: Option<String>,
}

/// POST /_matrix/client/v3/rooms/:roomId/report/:eventId
/// コンテンツ報告を記録する。
async fn report_event(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((room_id, event_id)): Path<(String, String)>,
    Json(body): Json<ReportBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::reports::create(
        &state.pool,
        &room_id,
        &event_id,
        &user.user_id,
        body.score,
        body.reason.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({})))
}
