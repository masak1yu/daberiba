use anyhow::Result;
use sqlx::MySqlPool;

/// account_data をセット（upsert）
/// room_id が空文字列ならグローバル、非空ならルーム固有
pub async fn set(
    pool: &MySqlPool,
    user_id: &str,
    room_id: &str,
    event_type: &str,
    content: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO account_data (user_id, room_id, event_type, content)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE content = VALUES(content)"#,
    )
    .bind(user_id)
    .bind(room_id)
    .bind(event_type)
    .bind(content)
    .execute(pool)
    .await?;
    Ok(())
}

/// account_data を 1 件取得
pub async fn get(
    pool: &MySqlPool,
    user_id: &str,
    room_id: &str,
    event_type: &str,
) -> Result<Option<serde_json::Value>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM account_data WHERE user_id = ? AND room_id = ? AND event_type = ?",
    )
    .bind(user_id)
    .bind(room_id)
    .bind(event_type)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(c,)| serde_json::from_str(&c).ok()))
}

/// /sync 用: ユーザーのすべての account_data を取得
/// (room_id, event_type, content_json) を返す。room_id が空文字列 = グローバル
pub async fn get_all_for_user(
    pool: &MySqlPool,
    user_id: &str,
) -> Result<Vec<(String, String, String)>> {
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT room_id, event_type, content FROM account_data WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}
