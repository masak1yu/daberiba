use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{extract::State, routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        // サードパーティプロトコル一覧（現状は空）
        .route(
            "/_matrix/client/v3/thirdparty/protocols",
            get(list_protocols),
        )
}

/// GET /_matrix/client/v3/thirdparty/protocols
///
/// 利用可能なサードパーティプロトコルを返す。
/// 現状はブリッジ未対応のため空オブジェクトを返す。
async fn list_protocols(
    State(_state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({})))
}
