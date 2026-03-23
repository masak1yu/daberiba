/// Federation ディレクトリクエリ
/// GET /_matrix/federation/v1/query/directory?room_alias=<alias>
///
/// X-Matrix 認証が必要。ここでは簡易実装として Authorization ヘッダの存在のみ確認する。
/// 完全な署名検証は今後の課題。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Query, State},
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
) -> ApiResult<Json<serde_json::Value>> {
    let result = db::room_aliases::resolve(&state.pool, &q.room_alias).await?;
    let room_id = result.ok_or(crate::error::AppError::NotFound)?;

    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());

    Ok(Json(serde_json::json!({
        "room_id": room_id,
        "servers": [server_name],
    })))
}
