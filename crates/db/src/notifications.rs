use anyhow::Result;
use sqlx::MySqlPool;

/// dispatch_push で notify アクションが発火した際に通知を記録する。
/// 重複挿入は無視する（同一 user_id + event_id が既に存在する場合）。
pub async fn record(
    pool: &MySqlPool,
    user_id: &str,
    room_id: &str,
    event_id: &str,
    notified_at: i64,
) -> Result<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO notifications (user_id, room_id, event_id, notified_at)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(room_id)
    .bind(event_id)
    .bind(notified_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct NotificationRow {
    pub id: u64,
    pub user_id: String,
    pub room_id: String,
    pub event_id: String,
    pub read_at: Option<i64>,
    pub notified_at: i64,
}

/// ユーザーの通知履歴を取得する（ページネーション: id 昇順）。
/// - from: この id より大きいものを返す（None = 先頭から）
/// - limit: 最大取得件数
pub async fn list(
    pool: &MySqlPool,
    user_id: &str,
    from: Option<u64>,
    limit: u32,
) -> Result<Vec<NotificationRow>> {
    use sqlx::Row;

    let from_id = from.unwrap_or(0);
    let rows = sqlx::query(
        r#"SELECT id, user_id, room_id, event_id, read_at, notified_at
           FROM notifications
           WHERE user_id = ? AND id > ?
           ORDER BY id ASC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(from_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(NotificationRow {
                id: row.try_get::<u64, _>("id")?,
                user_id: row.try_get("user_id")?,
                room_id: row.try_get("room_id")?,
                event_id: row.try_get("event_id")?,
                read_at: row.try_get("read_at")?,
                notified_at: row.try_get("notified_at")?,
            })
        })
        .collect()
}

/// receipt POST 時に、指定タイムスタンプ以前の通知を既読にする。
pub async fn mark_read_up_to(
    pool: &MySqlPool,
    user_id: &str,
    room_id: &str,
    up_to_ts: i64,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE notifications
           SET read_at = ?
           WHERE user_id = ? AND room_id = ? AND notified_at <= ? AND read_at IS NULL"#,
    )
    .bind(up_to_ts)
    .bind(user_id)
    .bind(room_id)
    .bind(up_to_ts)
    .execute(pool)
    .await?;
    Ok(())
}
