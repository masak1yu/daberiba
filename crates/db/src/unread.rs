use anyhow::Result;
use sqlx::MySqlPool;

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

    // ハイライト数: body フィールドまたは formatted_body フィールドに user_id が含まれるイベント
    // JSON_EXTRACT で body/formatted_body を抽出してから LIKE 検索することで
    // content 全体への誤ヒットを防ぐ
    let localpart = user_id
        .split(':')
        .next()
        .unwrap_or(user_id)
        .trim_start_matches('@');
    let highlight_row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM events
           WHERE room_id = ? AND stream_ordering > ? AND state_key IS NULL
             AND sender != ?
             AND (
               JSON_UNQUOTE(JSON_EXTRACT(content, '$.body')) LIKE CONCAT('%', ?, '%')
               OR JSON_UNQUOTE(JSON_EXTRACT(content, '$.formatted_body')) LIKE CONCAT('%', ?, '%')
             )"#,
    )
    .bind(room_id)
    .bind(since_ordering)
    .bind(user_id)
    .bind(localpart)
    .bind(localpart)
    .fetch_one(pool)
    .await?;

    Ok(UnreadCounts {
        notification_count: notification_row.0,
        highlight_count: highlight_row.0,
    })
}
