use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

pub async fn create(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");

    sqlx::query!(
        "INSERT INTO access_tokens (token, user_id, device_id) VALUES (?, ?, ?)",
        token,
        user_id,
        device_id
    )
    .execute(pool)
    .await?;

    Ok(token)
}

/// token を検証し (user_id, device_id) を返す
pub async fn verify(pool: &MySqlPool, token: &str) -> Result<Option<(String, String)>> {
    let row = sqlx::query!(
        "SELECT user_id, device_id FROM access_tokens WHERE token = ?",
        token
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| (r.user_id, r.device_id)))
}

pub async fn revoke(pool: &MySqlPool, token: &str) -> Result<()> {
    sqlx::query!("DELETE FROM access_tokens WHERE token = ?", token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_all(pool: &MySqlPool, user_id: &str) -> Result<()> {
    sqlx::query!("DELETE FROM access_tokens WHERE user_id = ?", user_id)
        .execute(pool)
        .await?;
    Ok(())
}
