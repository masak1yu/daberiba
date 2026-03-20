use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

pub async fn create(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");

    sqlx::query!(
        "INSERT INTO access_tokens (token, user_id, device_id) VALUES (?, ?, ?)",
        token,
        user_id,
        device_id,
    )
    .execute(pool)
    .await?;

    Ok(token)
}

pub async fn verify(pool: &MySqlPool, token: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        "SELECT user_id FROM access_tokens WHERE token = ?",
        token
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.user_id))
}
