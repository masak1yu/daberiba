use anyhow::Result;
use sqlx::MySqlPool;

#[derive(sqlx::FromRow)]
pub struct MediaRecord {
    pub media_id: String,
    pub server_name: String,
    pub user_id: String,
    pub content_type: String,
    pub filename: Option<String>,
    pub file_size: i64,
    pub room_id: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &MySqlPool,
    media_id: &str,
    server_name: &str,
    user_id: &str,
    content_type: &str,
    filename: Option<&str>,
    file_size: i64,
    room_id: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO media (media_id, server_name, user_id, content_type, filename, file_size, room_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        media_id,
        server_name,
        user_id,
        content_type,
        filename,
        file_size,
        room_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(
    pool: &MySqlPool,
    server_name: &str,
    media_id: &str,
) -> Result<Option<MediaRecord>> {
    let row = sqlx::query_as!(
        MediaRecord,
        "SELECT media_id, server_name, user_id, content_type, filename, file_size, room_id \
         FROM media WHERE server_name = ? AND media_id = ?",
        server_name,
        media_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// ユーザーがそのメディアにアクセス可能かチェックする。
/// room_id が NULL → 認証済みユーザー全員 OK（true を返す）。
/// room_id が設定されている → room_memberships で join しているか確認。
pub async fn is_accessible_by(
    pool: &MySqlPool,
    record: &MediaRecord,
    user_id: &str,
) -> Result<bool> {
    let Some(room_id) = &record.room_id else {
        return Ok(true);
    };
    let row = sqlx::query!(
        "SELECT 1 AS ok FROM room_memberships \
         WHERE room_id = ? AND user_id = ? AND membership = 'join'",
        room_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}
