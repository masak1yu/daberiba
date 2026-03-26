use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/publicRooms",
        get(get_public_rooms).post(post_public_rooms),
    )
}

#[derive(Deserialize)]
struct GetPublicRoomsParams {
    limit: Option<u64>,
    since: Option<String>,
    server: Option<String>,
    #[serde(rename = "filter")]
    filter: Option<String>,
}

#[derive(Deserialize)]
struct PostPublicRoomsBody {
    limit: Option<u64>,
    since: Option<String>,
    #[allow(dead_code)]
    server: Option<String>,
    filter: Option<PostPublicRoomsFilter>,
}

#[derive(Deserialize)]
struct PostPublicRoomsFilter {
    generic_search_term: Option<String>,
}

async fn get_public_rooms(
    State(state): State<AppState>,
    Query(params): Query<GetPublicRoomsParams>,
) -> ApiResult<Json<serde_json::Value>> {
    // 他サーバへのプロキシは未対応（自サーバのルームのみ返す）
    let _ = params.server;
    let limit = params.limit.unwrap_or(30).min(500);
    let offset = parse_since(params.since.as_deref());
    let filter = params.filter.as_deref();
    public_rooms_response(&state, filter, limit, offset).await
}

async fn post_public_rooms(
    State(state): State<AppState>,
    Json(body): Json<PostPublicRoomsBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let limit = body.limit.unwrap_or(30).min(500);
    let offset = parse_since(body.since.as_deref());
    let filter = body
        .filter
        .as_ref()
        .and_then(|f| f.generic_search_term.as_deref());
    public_rooms_response(&state, filter, limit, offset).await
}

fn parse_since(since: Option<&str>) -> u64 {
    since.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)
}

async fn public_rooms_response(
    state: &AppState,
    filter: Option<&str>,
    limit: u64,
    offset: u64,
) -> ApiResult<Json<serde_json::Value>> {
    let (rooms, total) = db::rooms::get_public_rooms(&state.pool, filter, limit, offset).await?;

    let next_batch = if offset + limit < total {
        Some((offset + limit).to_string())
    } else {
        None
    };

    let prev_batch = if offset > 0 {
        Some(offset.saturating_sub(limit).to_string())
    } else {
        None
    };

    let chunk: Vec<serde_json::Value> = rooms
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "room_id": r.room_id,
                "name": r.name,
                "topic": r.topic,
                "num_joined_members": r.num_joined_members,
                "world_readable": false,
                "guest_can_join": false,
            })
        })
        .collect();

    let mut resp = serde_json::json!({
        "chunk": chunk,
        "total_room_count_estimate": total,
    });

    if let Some(nb) = next_batch {
        resp["next_batch"] = serde_json::Value::String(nb);
    }
    if let Some(pb) = prev_batch {
        resp["prev_batch"] = serde_json::Value::String(pb);
    }

    Ok(Json(resp))
}
