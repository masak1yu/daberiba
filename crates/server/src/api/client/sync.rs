use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/sync", get(sync))
}

#[derive(Deserialize)]
struct SyncQuery {
    since: Option<String>,
    timeout: Option<u64>,
    filter: Option<String>,
}

async fn sync(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<SyncQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let result = db::sync::sync(&state.pool, &user.user_id, query.since.as_deref()).await?;
    Ok(Json(result))
}
