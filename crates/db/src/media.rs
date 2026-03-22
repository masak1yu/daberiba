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
}

pub async fn insert(
    pool: &MySqlPool,
    media_id: &str,
    server_name: &str,
    user_id: &str,
    content_type: &str,
    filename: Option<&str>,
    file_size: i64,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO media (media_id, server_name, user_id, content_type, filename, file_size) \
         VALUES (?, ?, ?, ?, ?, ?)",
        media_id,
        server_name,
        user_id,
        content_type,
        filename,
        file_size
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
        "SELECT media_id, server_name, user_id, content_type, filename, file_size \
         FROM media WHERE server_name = ? AND media_id = ?",
        server_name,
        media_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
