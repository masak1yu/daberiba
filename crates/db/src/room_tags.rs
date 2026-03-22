use anyhow::Result;
use sqlx::MySqlPool;

pub struct Tag {
    pub tag: String,
    pub order: Option<f64>,
}

/// ルームのタグ一覧を取得（GET /user/{userId}/rooms/{roomId}/tags）
pub async fn get_for_room(pool: &MySqlPool, user_id: &str, room_id: &str) -> Result<Vec<Tag>> {
    let rows: Vec<(String, Option<f64>)> =
        sqlx::query_as("SELECT tag, order_ FROM room_tags WHERE user_id = ? AND room_id = ?")
            .bind(user_id)
            .bind(room_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(tag, order)| Tag { tag, order })
        .collect())
}

/// タグをセット（PUT /user/{userId}/rooms/{roomId}/tags/{tag}）
pub async fn set(
    pool: &MySqlPool,
    user_id: &str,
    room_id: &str,
    tag: &str,
    order: Option<f64>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO room_tags (user_id, room_id, tag, order_)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE order_ = VALUES(order_)"#,
    )
    .bind(user_id)
    .bind(room_id)
    .bind(tag)
    .bind(order)
    .execute(pool)
    .await?;
    Ok(())
}

/// タグを削除（DELETE /user/{userId}/rooms/{roomId}/tags/{tag}）
pub async fn delete(pool: &MySqlPool, user_id: &str, room_id: &str, tag: &str) -> Result<()> {
    sqlx::query("DELETE FROM room_tags WHERE user_id = ? AND room_id = ? AND tag = ?")
        .bind(user_id)
        .bind(room_id)
        .bind(tag)
        .execute(pool)
        .await?;
    Ok(())
}

/// ユーザーの全タグを (room_id, Tag) で取得（/sync account_data 用）
pub async fn get_all_for_user(pool: &MySqlPool, user_id: &str) -> Result<Vec<(String, Tag)>> {
    let rows: Vec<(String, String, Option<f64>)> =
        sqlx::query_as("SELECT room_id, tag, order_ FROM room_tags WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(room_id, tag, order)| (room_id, Tag { tag, order }))
        .collect())
}
