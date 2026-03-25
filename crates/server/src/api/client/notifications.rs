use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/notifications", get(get_notifications))
}

#[derive(Deserialize)]
struct NotificationsQuery {
    /// ページネーショントークン（前ページ末尾の notification id の文字列表現）
    from: Option<String>,
    /// 取得件数（デフォルト 20、最大 100）
    limit: Option<u32>,
    /// "highlight" のみフィルタ
    only: Option<String>,
}

#[derive(Serialize)]
struct NotificationsResponse {
    notifications: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

/// GET /_matrix/client/v3/notifications
/// プッシュ通知履歴を返す。only=highlight の場合はハイライトのみ。
async fn get_notifications(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<NotificationsQuery>,
) -> ApiResult<Json<NotificationsResponse>> {
    let from_id: Option<u64> = query.from.as_deref().and_then(|s| s.parse().ok());
    // limit+1 件取得してページ継続を判定する
    let fetch_limit = query.limit.unwrap_or(20).min(100) + 1;

    let rows = db::notifications::list(&state.pool, &user.user_id, from_id, fetch_limit).await?;

    let has_more = rows.len() as u32 == fetch_limit;
    let rows = if has_more {
        &rows[..rows.len() - 1]
    } else {
        &rows[..]
    };

    let only_highlight = query.only.as_deref() == Some("highlight");

    let mut notifications = Vec::new();
    for row in rows {
        // only=highlight の場合は unread_highlights テーブルで確認
        if only_highlight {
            let is_hl =
                db::unread::is_highlight(&state.pool, &row.room_id, &user.user_id, &row.event_id)
                    .await
                    .unwrap_or(false);
            if !is_hl {
                continue;
            }
        }

        // イベント本体を取得
        let event = db::events::get_by_id(&state.pool, &row.event_id)
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        notifications.push(serde_json::json!({
            "room_id": row.room_id,
            "event": event,
            "read": row.read_at.is_some(),
            "ts": row.notified_at,
        }));
    }

    let next_token = if has_more {
        rows.last().map(|r| r.id.to_string())
    } else {
        None
    };

    Ok(Json(NotificationsResponse {
        notifications,
        next_token,
    }))
}
