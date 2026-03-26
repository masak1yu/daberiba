use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/rooms/{roomId}/initialSync",
        get(room_initial_sync),
    )
}

/// GET /_matrix/client/v3/rooms/{roomId}/initialSync
/// レガシーエンドポイント。ルームの現在の状態・メッセージ・メンバーシップを一括返却する。
async fn room_initial_sync(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let membership = db::rooms::get_membership(&state.pool, &room_id, &user.user_id)
        .await?
        .unwrap_or_else(|| "leave".to_string());

    // 現在の状態スナップショット
    let state_events = db::room_state::get_all(&state.pool, &room_id)
        .await
        .unwrap_or_default();

    // 最新メッセージ（50 件、後方向き）
    let page = db::events::get_messages(&state.pool, &room_id, None, "b", 50)
        .await
        .unwrap_or(db::events::MessagePage {
            events: vec![],
            start: "s0".to_string(),
            end: None,
        });

    // 既読レシート
    let receipts = db::receipts::get_for_room(&state.pool, &room_id)
        .await
        .map(|r| {
            if r.is_empty() {
                vec![]
            } else {
                vec![db::receipts::to_event(r)]
            }
        })
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "room_id": room_id,
        "membership": membership,
        "state": state_events,
        "messages": {
            "chunk": page.events,
            "start": page.start,
            "end": page.end,
        },
        "receipts": receipts,
        "presence": [],
    })))
}
