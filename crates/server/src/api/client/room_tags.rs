use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v3/user/:userId/rooms/:roomId/tags",
            get(get_tags),
        )
        .route(
            "/_matrix/client/v3/user/:userId/rooms/:roomId/tags/:tag",
            put(set_tag).delete(delete_tag),
        )
}

#[derive(Deserialize)]
struct RoomPath {
    #[serde(rename = "userId")]
    _user_id: String,
    #[serde(rename = "roomId")]
    room_id: String,
}

#[derive(Deserialize)]
struct TagPath {
    #[serde(rename = "userId")]
    _user_id: String,
    #[serde(rename = "roomId")]
    room_id: String,
    tag: String,
}

#[derive(Deserialize)]
struct TagBody {
    order: Option<f64>,
}

#[derive(Serialize)]
struct TagsResponse {
    tags: HashMap<String, TagContent>,
}

#[derive(Serialize)]
struct TagContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<f64>,
}

async fn get_tags(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<RoomPath>,
) -> ApiResult<Json<TagsResponse>> {
    let tags = db::room_tags::get_for_room(&state.pool, &user.user_id, &path.room_id).await?;
    let mut map = HashMap::new();
    for t in tags {
        map.insert(t.tag, TagContent { order: t.order });
    }
    Ok(Json(TagsResponse { tags: map }))
}

async fn set_tag(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<TagPath>,
    Json(body): Json<TagBody>,
) -> ApiResult<StatusCode> {
    db::room_tags::set(
        &state.pool,
        &user.user_id,
        &path.room_id,
        &path.tag,
        body.order,
    )
    .await?;
    Ok(StatusCode::OK)
}

async fn delete_tag(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<TagPath>,
) -> ApiResult<StatusCode> {
    db::room_tags::delete(&state.pool, &user.user_id, &path.room_id, &path.tag).await?;
    Ok(StatusCode::OK)
}
