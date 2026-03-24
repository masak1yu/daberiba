/// Federation ディレクトリクエリ
/// GET /_matrix/federation/v1/query/directory?room_alias=<alias>
///
/// X-Matrix 署名検証済みリクエストのみ受け付ける。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, Uri},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/federation/v1/query/directory",
        get(query_directory),
    )
}

#[derive(Deserialize)]
struct DirectoryQuery {
    room_alias: String,
}

async fn query_directory(
    State(state): State<AppState>,
    Query(q): Query<DirectoryQuery>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<serde_json::Value>> {
    crate::xmatrix::verify_request(&state, &headers, "GET", &uri, None).await?;

    let result = db::room_aliases::resolve(&state.pool, &q.room_alias).await?;
    let room_id = result.ok_or(crate::error::AppError::NotFound)?;

    Ok(Json(serde_json::json!({
        "room_id": room_id,
        "servers": [&*state.server_name],
    })))
}
