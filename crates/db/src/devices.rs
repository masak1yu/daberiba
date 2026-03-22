use anyhow::Result;
use sqlx::MySqlPool;

#[derive(sqlx::FromRow)]
pub struct Device {
    pub device_id: String,
    pub display_name: Option<String>,
    pub last_seen_ts: Option<i64>,
    pub last_seen_ip: Option<String>,
}

pub async fn create(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<()> {
    sqlx::query!(
        "INSERT IGNORE INTO devices (device_id, user_id) VALUES (?, ?)",
        device_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &MySqlPool, user_id: &str) -> Result<Vec<Device>> {
    let rows = sqlx::query_as!(
        Device,
        "SELECT device_id, display_name, last_seen_ts, last_seen_ip \
         FROM devices WHERE user_id = ? ORDER BY created_at ASC",
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<Option<Device>> {
    let row = sqlx::query_as!(
        Device,
        "SELECT device_id, display_name, last_seen_ts, last_seen_ip \
         FROM devices WHERE user_id = ? AND device_id = ?",
        user_id,
        device_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_display_name(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    display_name: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query!(
        "UPDATE devices SET display_name = ? WHERE user_id = ? AND device_id = ?",
        display_name,
        user_id,
        device_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn update_last_seen(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    last_seen_ts: i64,
    last_seen_ip: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE devices SET last_seen_ts = ?, last_seen_ip = ? \
         WHERE user_id = ? AND device_id = ?",
        last_seen_ts,
        last_seen_ip,
        user_id,
        device_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &MySqlPool, user_id: &str, device_id: &str) -> Result<bool> {
    sqlx::query!(
        "DELETE FROM access_tokens WHERE user_id = ? AND device_id = ?",
        user_id,
        device_id
    )
    .execute(pool)
    .await?;

    let result = sqlx::query!(
        "DELETE FROM devices WHERE user_id = ? AND device_id = ?",
        user_id,
        device_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_many(pool: &MySqlPool, user_id: &str, device_ids: &[String]) -> Result<()> {
    for device_id in device_ids {
        delete(pool, user_id, device_id).await?;
    }
    Ok(())
}
