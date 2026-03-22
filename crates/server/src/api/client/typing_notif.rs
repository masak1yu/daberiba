use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/rooms/{roomId}/typing/{userId}",
        put(set_typing),
    )
}

#[derive(Deserialize)]
struct TypingPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "userId")]
    _user_id: String,
}

#[derive(Deserialize)]
struct TypingBody {
    typing: bool,
    /// タイムアウト（ミリ秒）。typing=true の場合のみ使用。デフォルト 30 秒。
    timeout: Option<u64>,
}

async fn set_typing(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<TypingPath>,
    Json(body): Json<TypingBody>,
) -> ApiResult<StatusCode> {
    if body.typing {
        let timeout = body.timeout.unwrap_or(30_000);
        state.typing.set(&path.room_id, &user.user_id, timeout);
    } else {
        state.typing.unset(&path.room_id, &user.user_id);
    }
    Ok(StatusCode::OK)
}
