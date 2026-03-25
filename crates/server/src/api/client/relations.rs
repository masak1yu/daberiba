use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v1/rooms/{roomId}/relations/{eventId}",
            get(get_relations),
        )
        .route(
            "/_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}",
            get(get_relations_by_type),
        )
        .route(
            "/_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}/{eventType}",
            get(get_relations_by_type_and_event_type),
        )
}

#[derive(Deserialize)]
struct RelationsQuery {
    from: Option<String>,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct RelationsResponse {
    chunk: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_batch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev_batch: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}
async fn get_relations(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path((room_id, event_id)): Path<(String, String)>,
    Query(query): Query<RelationsQuery>,
) -> ApiResult<Json<RelationsResponse>> {
    fetch_relations(state, room_id, event_id, None, None, query).await
}

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}
async fn get_relations_by_type(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path((room_id, event_id, rel_type)): Path<(String, String, String)>,
    Query(query): Query<RelationsQuery>,
) -> ApiResult<Json<RelationsResponse>> {
    fetch_relations(state, room_id, event_id, Some(rel_type), None, query).await
}

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}/{eventType}
async fn get_relations_by_type_and_event_type(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path((room_id, event_id, rel_type, event_type)): Path<(String, String, String, String)>,
    Query(query): Query<RelationsQuery>,
) -> ApiResult<Json<RelationsResponse>> {
    fetch_relations(
        state,
        room_id,
        event_id,
        Some(rel_type),
        Some(event_type),
        query,
    )
    .await
}

async fn fetch_relations(
    state: AppState,
    room_id: String,
    event_id: String,
    rel_type: Option<String>,
    event_type: Option<String>,
    query: RelationsQuery,
) -> ApiResult<Json<RelationsResponse>> {
    let limit = query.limit.unwrap_or(20).min(50);

    let page = db::relations::list(
        &state.pool,
        &room_id,
        &event_id,
        rel_type.as_deref(),
        event_type.as_deref(),
        query.from.as_deref(),
        limit,
    )
    .await?;

    Ok(Json(RelationsResponse {
        chunk: page.events,
        next_batch: page.next_batch,
        prev_batch: page.prev_batch,
    }))
}
