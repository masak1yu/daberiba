use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/events", get(get_global_events))
}

#[derive(Deserialize)]
struct GlobalEventsQuery {
    from: Option<String>,
    /// タイムアウト（ミリ秒）。現在はポーリングせずに即時返却。
    #[allow(dead_code)]
    timeout: Option<u64>,
    room_id: Option<String>,
}

/// レガシーグローバルイベントストリーム。
///
/// `from` トークン以降にユーザーが参加しているルームで発生したイベントを返す。
/// `timeout` パラメータは受け付けるが、現実装では即時返却（long-poll なし）。
async fn get_global_events(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<GlobalEventsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let since = query
        .from
        .as_deref()
        .and_then(db::events::parse_token)
        .unwrap_or(0);

    let (events, max_ordering) = db::events::get_global_events_since(
        &state.pool,
        &user.user_id,
        since,
        query.room_id.as_deref(),
        100,
    )
    .await?;

    let start = db::events::ordering_to_token(since);
    let end = max_ordering
        .map(db::events::ordering_to_token)
        .unwrap_or_else(|| start.clone());

    Ok(Json(serde_json::json!({
        "start": start,
        "end": end,
        "chunk": events,
    })))
}
