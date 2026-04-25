use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/rooms/:roomId/read_markers",
        post(set_read_markers),
    )
}

#[derive(Deserialize)]
struct ReadMarkersBody {
    /// m.read: 既読マーカー（通知カウントに影響）
    #[serde(rename = "m.read")]
    m_read: Option<String>,
    /// m.read.private: プライベート既読マーカー（他ユーザーに非公開）
    #[serde(rename = "m.read.private")]
    m_read_private: Option<String>,
    /// m.fully_read: 完全既読マーカー（account_data に保存）
    #[serde(rename = "m.fully_read")]
    m_fully_read: Option<String>,
}

/// POST /_matrix/client/v3/rooms/:roomId/read_markers
/// m.read / m.read.private / m.fully_read を一括で設定する。
async fn set_read_markers(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Json(body): Json<ReadMarkersBody>,
) -> ApiResult<StatusCode> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    // m.read を receipts に記録し、通知・ハイライトをクリア
    if let Some(event_id) = &body.m_read {
        db::receipts::upsert(&state.pool, &room_id, &user.user_id, "m.read", event_id).await?;
        let _ =
            db::notifications::mark_read_up_to(&state.pool, &user.user_id, &room_id, now_ms).await;
        if let Ok(Some(ordering)) = db::events::get_stream_ordering(&state.pool, event_id).await {
            let _ =
                db::unread::delete_highlights_up_to(&state.pool, &room_id, &user.user_id, ordering)
                    .await;
        }
    }

    // m.read.private を receipts に記録（同様にクリア）
    if let Some(event_id) = &body.m_read_private {
        db::receipts::upsert(
            &state.pool,
            &room_id,
            &user.user_id,
            "m.read.private",
            event_id,
        )
        .await?;
    }

    // m.fully_read は room account_data に保存
    if let Some(event_id) = &body.m_fully_read {
        let content =
            serde_json::to_string(&serde_json::json!({ "event_id": event_id })).unwrap_or_default();
        db::account_data::set(
            &state.pool,
            &user.user_id,
            &room_id,
            "m.fully_read",
            &content,
        )
        .await?;
    }

    Ok(StatusCode::OK)
}
