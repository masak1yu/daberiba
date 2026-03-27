use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/user_directory/search",
        post(search_users),
    )
}

#[derive(Deserialize)]
struct SearchBody {
    search_term: String,
    limit: Option<u64>,
}

/// POST /_matrix/client/v3/user_directory/search
///
/// ユーザーディレクトリを検索する。
/// user_id または display_name に `search_term` が含まれるユーザーを返す。
async fn search_users(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Json(body): Json<SearchBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let limit = body.limit.unwrap_or(10).min(50);
    let results = db::users::search_directory(&state.pool, &body.search_term, limit).await?;

    let results_json: Vec<serde_json::Value> = results
        .into_iter()
        .map(|r| {
            let mut obj = serde_json::json!({ "user_id": r.user_id });
            if let Some(dn) = r.display_name {
                obj["display_name"] = serde_json::Value::String(dn);
            }
            if let Some(av) = r.avatar_url {
                obj["avatar_url"] = serde_json::Value::String(av);
            }
            obj
        })
        .collect();

    let limited = results_json.len() >= limit as usize;

    Ok(Json(serde_json::json!({
        "results": results_json,
        "limited": limited,
    })))
}
