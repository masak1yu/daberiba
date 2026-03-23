/// Federation get event
/// GET /_matrix/federation/v1/event/{eventId}
///
/// 指定した event_id のイベントを PDU 形式で返す。
/// バックフィル（過去イベント取得）に使用される。
use crate::{error::ApiResult, error::AppError, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::get,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/federation/v1/event/:event_id", get(get_event))
}

async fn get_event(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "GET", &uri, None).await?;

    let event = db::events::get_by_id(&state.pool, &event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let origin_server_ts = event["origin_server_ts"].as_i64().unwrap_or(0);

    Ok(Json(serde_json::json!({
        "origin": server_name,
        "origin_server_ts": origin_server_ts,
        "pdus": [event],
    })))
}
