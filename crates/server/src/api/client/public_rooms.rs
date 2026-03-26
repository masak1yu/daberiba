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
    // ?server= が指定された場合は federation 経由でリモートサーバにプロキシ
    if let Some(server) = params.server.as_deref() {
        return proxy_to_server(
            &state,
            server,
            params.limit,
            params.since.as_deref(),
            params.filter.as_deref(),
        )
        .await;
    }
    let limit = params.limit.unwrap_or(30).min(500);
    let offset = parse_since(params.since.as_deref());
    let filter = params.filter.as_deref();
    public_rooms_response(&state, filter, limit, offset).await
}

async fn post_public_rooms(
    State(state): State<AppState>,
    Json(body): Json<PostPublicRoomsBody>,
) -> ApiResult<Json<serde_json::Value>> {
    // body.server が指定された場合はリモートサーバにプロキシ
    if let Some(server) = body.server.as_deref() {
        let filter = body
            .filter
            .as_ref()
            .and_then(|f| f.generic_search_term.as_deref());
        return proxy_to_server(&state, server, body.limit, body.since.as_deref(), filter).await;
    }
    let limit = body.limit.unwrap_or(30).min(500);
    let offset = parse_since(body.since.as_deref());
    let filter = body
        .filter
        .as_ref()
        .and_then(|f| f.generic_search_term.as_deref());
    public_rooms_response(&state, filter, limit, offset).await
}

/// 指定サーバーの `/_matrix/federation/v1/publicRooms` にプロキシする。
async fn proxy_to_server(
    state: &AppState,
    server: &str,
    limit: Option<u64>,
    since: Option<&str>,
    filter: Option<&str>,
) -> ApiResult<Json<serde_json::Value>> {
    use crate::error::AppError;

    let mut url = format!("https://{}/_matrix/federation/v1/publicRooms", server);
    let mut sep = '?';
    if let Some(l) = limit {
        url.push_str(&format!("{}limit={}", sep, l));
        sep = '&';
    }
    if let Some(s) = since {
        url.push_str(&format!("{}since={}", sep, s));
        sep = '&';
    }
    if let Some(f) = filter {
        url.push_str(&format!("{}filter={}", sep, percent_encode(f)));
    }

    let resp = state
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(body))
}

/// URL パーセントエンコード（スペース・特殊文字のみ簡易エスケープ）。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
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
