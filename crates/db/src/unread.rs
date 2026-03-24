use anyhow::Result;
use sqlx::MySqlPool;

/// dispatch_push でハイライトと判定されたイベントを記録する。
/// 重複挿入は無視する（INSERT IGNORE）。
pub async fn record_highlight(
    pool: &MySqlPool,
    room_id: &str,
    user_id: &str,
    event_id: &str,
    stream_ordering: i64,
) -> Result<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO unread_highlights (room_id, user_id, event_id, stream_ordering)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(room_id)
    .bind(user_id)
    .bind(event_id)
    .bind(stream_ordering)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct UnreadCounts {
    pub notification_count: i64,
    pub highlight_count: i64,
}

/// ルームの未読カウントを計算
/// - notification_count: ユーザーの最終既読イベント以降のタイムラインイベント数
/// - highlight_count: 同範囲でユーザーが mention されているイベント数
pub async fn get_for_room(pool: &MySqlPool, room_id: &str, user_id: &str) -> Result<UnreadCounts> {
    // 最終既読イベントの stream_ordering を取得
    let last_read_row: Option<(i64,)> = sqlx::query_as(
        r#"SELECT e.stream_ordering
           FROM receipts r
           INNER JOIN events e ON e.event_id = r.event_id
           WHERE r.room_id = ? AND r.user_id = ? AND r.receipt_type = 'm.read'
           LIMIT 1"#,
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let since_ordering: i64 = last_read_row.map(|(o,)| o).unwrap_or(0);

    // 未読イベント数（タイムラインイベント = state_key IS NULL）
    let notification_row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM events
           WHERE room_id = ? AND stream_ordering > ? AND state_key IS NULL AND sender != ?"#,
    )
    .bind(room_id)
    .bind(since_ordering)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    // ハイライト数: push rule 評価でハイライトと記録されたイベント数
    // （dispatch_push が unread_highlights に挿入したもの）
    let highlight_row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM unread_highlights
           WHERE room_id = ? AND user_id = ? AND stream_ordering > ?"#,
    )
    .bind(room_id)
    .bind(user_id)
    .bind(since_ordering)
    .fetch_one(pool)
    .await?;

    Ok(UnreadCounts {
        notification_count: notification_row.0,
        highlight_count: highlight_row.0,
    })
}
