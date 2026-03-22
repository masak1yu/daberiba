use anyhow::Result;
use sqlx::MySqlPool;

pub async fn get(pool: &MySqlPool, user_id: &str) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query!(
        "SELECT display_name, avatar_url FROM users WHERE user_id = ?",
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        serde_json::json!({
            "displayname": r.display_name,
            "avatar_url": r.avatar_url,
        })
    }))
}

pub async fn set_displayname(
    pool: &MySqlPool,
    user_id: &str,
    displayname: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE users SET display_name = ? WHERE user_id = ?",
        displayname,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_avatar_url(
    pool: &MySqlPool,
    user_id: &str,
    avatar_url: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE users SET avatar_url = ? WHERE user_id = ?",
        avatar_url,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
