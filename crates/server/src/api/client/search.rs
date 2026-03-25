use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/search", post(search))
}

#[derive(Deserialize)]
struct SearchQuery {
    /// ページネーショントークン（前ページ末尾の stream_ordering の文字列表現）
    next_batch: Option<String>,
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
/// ?next_batch=<stream_ordering> でページネーション。
async fn search(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<SearchQuery>,
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

    // next_batch を stream_ordering カーソルとして解釈
    let before_ordering: Option<i64> = query.next_batch.as_deref().and_then(|s| s.parse().ok());

    // limit+1 件取得してページ継続を判定する
    let results = db::events::search_room_events(
        &state.pool,
        &user.user_id,
        &criteria.search_term,
        criteria.filter.as_ref().and_then(|f| f.rooms.as_deref()),
        criteria.order_by.as_deref(),
        before_ordering,
        limit + 1,
    )
    .await?;

    let has_more = results.len() as i64 == limit + 1;
    let results: Vec<_> = results.into_iter().take(limit as usize).collect();

    let next_batch = if has_more {
        results
            .last()
            .and_then(|e| e.get("stream_ordering"))
            .and_then(|v| v.as_i64())
            .map(|o| o.to_string())
    } else {
        None
    };

    let count = results.len() as i64;
    let results = results
        .into_iter()
        .enumerate()
        .map(|(i, mut event)| {
            // stream_ordering はクライアントに返さない（内部カーソル用）
            if let Some(obj) = event.as_object_mut() {
                obj.remove("stream_ordering");
            }
            SearchResult {
                rank: 1.0 - (i as f64 / (count as f64 + 1.0)),
                result: event,
            }
        })
        .collect();

    Ok(Json(SearchResponse {
        search_categories: SearchResultCategories {
            room_events: RoomEventResults {
                count,
                results,
                next_batch,
            },
        },
    }))
}
