use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/rooms/:roomId/receipt/:receiptType/:eventId",
        post(send_receipt),
    )
}

struct ReceiptPath {
    room_id: String,
    receipt_type: String,
    event_id: String,
}

impl<'de> serde::Deserialize<'de> for ReceiptPath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Raw {
            #[serde(rename = "roomId")]
            room_id: String,
            #[serde(rename = "receiptType")]
            receipt_type: String,
            #[serde(rename = "eventId")]
            event_id: String,
        }
        let r = Raw::deserialize(d)?;
        Ok(ReceiptPath {
            room_id: r.room_id,
            receipt_type: r.receipt_type,
            event_id: r.event_id,
        })
    }
}

async fn send_receipt(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<ReceiptPath>,
    _body: Option<Json<serde_json::Value>>,
) -> ApiResult<StatusCode> {
    db::receipts::upsert(
        &state.pool,
        &path.room_id,
        &user.user_id,
        &path.receipt_type,
        &path.event_id,
    )
    .await?;

    // m.read / m.read.private の場合は通知を既読にし、ハイライトをクリアする
    if path.receipt_type == "m.read" || path.receipt_type == "m.read.private" {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let _ =
            db::notifications::mark_read_up_to(&state.pool, &user.user_id, &path.room_id, now_ms)
                .await;

        // event_id の stream_ordering を取得してハイライトをクリア
        if let Ok(Some(ordering)) =
            db::events::get_stream_ordering(&state.pool, &path.event_id).await
        {
            let _ = db::unread::delete_highlights_up_to(
                &state.pool,
                &path.room_id,
                &user.user_id,
                ordering,
            )
            .await;
        }
    }

    Ok(StatusCode::OK)
}
