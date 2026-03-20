use anyhow::Result;
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

pub async fn create(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");

    sqlx::query(
        "INSERT INTO access_tokens (token, user_id, device_id) VALUES (?, ?, ?)",
    )
    .bind(&token)
    .bind(user_id)
    .bind(device_id)
    .execute(pool)
    .await?;

    Ok(token)
}

/// token を検証し (user_id, device_id) を返す
pub async fn verify(pool: &MySqlPool, token: &str) -> Result<Option<(String, String)>> {
    let row = sqlx::query(
        "SELECT user_id, device_id FROM access_tokens WHERE token = ?",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        (r.get::<String, _>("user_id"), r.get::<String, _>("device_id"))
    }))
}

pub async fn revoke(pool: &MySqlPool, token: &str) -> Result<()> {
    sqlx::query("DELETE FROM access_tokens WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_all(pool: &MySqlPool, user_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM access_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
