/// Federation backfill
/// GET /_matrix/federation/v1/backfill/{roomId}
///
/// 指定ルームの過去イベントを PDU 形式で返す。
/// クエリパラメータ:
///   v    : 起点となる event_id（複数指定可）
///   limit: 取得件数（省略時 10、最大 100）
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Uri},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/federation/v1/backfill/:room_id", get(backfill))
}

#[derive(Deserialize)]
struct BackfillQuery {
    /// 起点イベント ID（複数指定可）
    v: Option<Vec<String>>,
    limit: Option<u32>,
}

async fn backfill(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(q): Query<BackfillQuery>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "GET", &uri, None).await?;

    let limit = q.limit.unwrap_or(10).min(100);
    let from_ids = q.v.unwrap_or_default();

    let pdus = db::events::get_backfill(&state.pool, &room_id, &from_ids, limit)
        .await
        .map_err(crate::error::AppError::Internal)?;

    Ok(Json(serde_json::json!({
        "origin": &*state.server_name,
        "origin_server_ts": chrono::Utc::now().timestamp_millis(),
        "pdus": pdus,
    })))
}
