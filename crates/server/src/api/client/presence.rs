use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/presence/:userId/status",
        put(set_presence).get(get_presence),
    )
}

#[derive(Deserialize)]
struct PresenceBody {
    presence: String,
    status_msg: Option<String>,
}

#[derive(Serialize)]
struct PresenceResponse {
    presence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_msg: Option<String>,
    last_active_ago: i64,
    currently_active: bool,
}

async fn set_presence(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(user_id): Path<String>,
    Json(body): Json<PresenceBody>,
) -> ApiResult<StatusCode> {
    // 自分以外のプレゼンスは設定不可
    if user_id != user.user_id {
        return Err(AppError::Forbidden);
    }

    let valid = matches!(body.presence.as_str(), "online" | "offline" | "unavailable");
    if !valid {
        return Err(AppError::BadRequest("invalid presence value".into()));
    }

    db::presence::set(
        &state.pool,
        &user.user_id,
        &body.presence,
        body.status_msg.as_deref(),
    )
    .await?;

    Ok(StatusCode::OK)
}

async fn get_presence(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<PresenceResponse>> {
    let status = db::presence::get(&state.pool, &user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let last_active_ago = now_ms - status.last_active_ts;
    let currently_active = status.presence == "online" && last_active_ago < 60_000;

    Ok(Json(PresenceResponse {
        presence: status.presence,
        status_msg: status.status_msg,
        last_active_ago,
        currently_active,
    }))
}
