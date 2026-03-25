use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v1/rooms/{roomId}/timestamp_to_event",
        get(timestamp_to_event),
    )
}

#[derive(Deserialize)]
struct TimestampQuery {
    /// Unix タイムスタンプ（ミリ秒）
    ts: i64,
    /// "f"（ts 以降で最古）または "b"（ts 以前で最新）。デフォルト "f"
    dir: Option<String>,
}

#[derive(Serialize)]
struct TimestampResponse {
    event_id: String,
    origin_server_ts: i64,
}

/// GET /_matrix/client/v1/rooms/{roomId}/timestamp_to_event
/// 指定タイムスタンプに最も近いイベントを返す（MSC3030）。
async fn timestamp_to_event(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(query): Query<TimestampQuery>,
) -> ApiResult<Json<TimestampResponse>> {
    let dir = query.dir.as_deref().unwrap_or("f");

    let result = db::events::get_closest_event(&state.pool, &room_id, query.ts, dir).await?;
    let (event_id, origin_server_ts) = result.ok_or(AppError::NotFound)?;

    Ok(Json(TimestampResponse {
        event_id,
        origin_server_ts,
    }))
}
