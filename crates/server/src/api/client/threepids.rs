use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/account/3pids", get(list_threepids))
        .route("/_matrix/client/v3/account/3pid/add", post(add_threepid))
        .route(
            "/_matrix/client/v3/account/3pid/delete",
            post(delete_threepid),
        )
}

/// GET /_matrix/client/v3/account/3pids
async fn list_threepids(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let threepids = db::threepids::list(&state.pool, &user.user_id).await?;
    Ok(Json(serde_json::json!({ "threepids": threepids })))
}

#[derive(Deserialize)]
struct AddThreepidBody {
    medium: String,
    address: String,
    /// バリデーショントークン（本実装では検証なし・将来の拡張用）
    #[serde(default)]
    _client_secret: Option<String>,
    #[serde(default)]
    _sid: Option<String>,
}

/// POST /_matrix/client/v3/account/3pid/add
/// identity server による実際のバリデーションは行わず、直接登録する。
async fn add_threepid(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<AddThreepidBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::threepids::add(&state.pool, &user.user_id, &body.medium, &body.address).await?;
    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct DeleteThreepidBody {
    medium: String,
    address: String,
    #[serde(default)]
    id_server: Option<String>,
}

/// POST /_matrix/client/v3/account/3pid/delete
async fn delete_threepid(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<DeleteThreepidBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::threepids::delete(&state.pool, &user.user_id, &body.medium, &body.address).await?;
    // id_server_unbind_result: no-support（外部 identity server は不使用）
    let _ = body.id_server;
    Ok(Json(
        serde_json::json!({ "id_server_unbind_result": "no-support" }),
    ))
}
