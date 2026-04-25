use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v3/user/:userId/filter",
            post(create_filter),
        )
        .route(
            "/_matrix/client/v3/user/:userId/filter/:filterId",
            get(get_filter),
        )
}

#[derive(Deserialize)]
struct FilterIdPath {
    #[serde(rename = "userId")]
    _user_id: String,
    #[serde(rename = "filterId")]
    filter_id: String,
}

async fn create_filter(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let filter_json =
        serde_json::to_string(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let filter_id = db::filters::create(&state.pool, &user.user_id, &filter_json).await?;
    Ok(Json(
        serde_json::json!({ "filter_id": filter_id.to_string() }),
    ))
}

async fn get_filter(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<FilterIdPath>,
) -> ApiResult<Json<serde_json::Value>> {
    let filter_id: u64 = path.filter_id.parse().unwrap_or(0);
    let filter = db::filters::get(&state.pool, &user.user_id, filter_id).await?;
    match filter {
        Some(f) => Ok(Json(
            serde_json::from_str(&f).map_err(|e| AppError::Internal(e.into()))?,
        )),
        None => Err(AppError::NotFound),
    }
}
