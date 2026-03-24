use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/search", post(search))
}

#[derive(Deserialize)]
struct SearchRequest {
    search_categories: SearchCategories,
}

#[derive(Deserialize)]
struct SearchCategories {
    #[serde(default)]
    room_events: Option<RoomEventCriteria>,
}

#[derive(Deserialize)]
struct RoomEventCriteria {
    search_term: String,
    #[serde(default)]
    filter: Option<SearchFilter>,
    #[serde(default)]
    order_by: Option<String>,
}

#[derive(Deserialize)]
struct SearchFilter {
    #[serde(default)]
    rooms: Option<Vec<String>>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Serialize)]
struct SearchResponse {
    search_categories: SearchResultCategories,
}

#[derive(Serialize)]
struct SearchResultCategories {
    room_events: RoomEventResults,
}

#[derive(Serialize)]
struct RoomEventResults {
    count: i64,
    results: Vec<SearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_batch: Option<String>,
}

#[derive(Serialize)]
struct SearchResult {
    rank: f64,
    result: serde_json::Value,
}

/// POST /_matrix/client/v3/search
/// ルームイベントの全文検索（MariaDB LIKE ベース）。
async fn search(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<SearchRequest>,
) -> ApiResult<Json<SearchResponse>> {
    let Some(criteria) = body.search_categories.room_events else {
        return Ok(Json(SearchResponse {
            search_categories: SearchResultCategories {
                room_events: RoomEventResults {
                    count: 0,
                    results: vec![],
                    next_batch: None,
                },
            },
        }));
    };

    let limit = criteria
        .filter
        .as_ref()
        .and_then(|f| f.limit)
        .unwrap_or(10)
        .min(100) as i64;

    let results = db::events::search_room_events(
        &state.pool,
        &user.user_id,
        &criteria.search_term,
        criteria.filter.as_ref().and_then(|f| f.rooms.as_deref()),
        criteria.order_by.as_deref(),
        limit,
    )
    .await?;

    let count = results.len() as i64;
    let results = results
        .into_iter()
        .enumerate()
        .map(|(i, event)| SearchResult {
            // rank は単純に逆順インデックス（関連度スコアなし）
            rank: 1.0 - (i as f64 / (count as f64 + 1.0)),
            result: event,
        })
        .collect();

    Ok(Json(SearchResponse {
        search_categories: SearchResultCategories {
            room_events: RoomEventResults {
                count,
                results,
                next_batch: None,
            },
        },
    }))
}
