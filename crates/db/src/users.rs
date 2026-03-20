use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

pub async fn register(
    pool: &MySqlPool,
    username: &str,
    password: &str,
    server_name: &str,
) -> Result<(String, String, String)> {
    let user_id = format!("@{}:{}", username, server_name);
    let password_hash = hash_password(password)?;
    let device_id = Uuid::new_v4().to_string().to_uppercase()[..8].to_string();

    sqlx::query!(
        "INSERT INTO users (user_id, password_hash) VALUES (?, ?)",
        user_id,
        password_hash,
    )
    .execute(pool)
    .await?;

    let access_token = crate::access_tokens::create(pool, &user_id, &device_id).await?;

    Ok((user_id, access_token, device_id))
}

pub async fn login(
    pool: &MySqlPool,
    username: &str,
    password: &str,
    server_name: &str,
) -> Result<(String, String, String)> {
    let user_id = if username.starts_with('@') {
        username.to_string()
    } else {
        format!("@{}:{}", username, server_name)
    };

    let row = sqlx::query!("SELECT password_hash FROM users WHERE user_id = ?", user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid credentials"))?;

    verify_password(password, &row.password_hash)?;

    let device_id = Uuid::new_v4().to_string().to_uppercase()[..8].to_string();
    let access_token = crate::access_tokens::create(pool, &user_id, &device_id).await?;

    Ok((user_id, access_token, device_id))
}

fn hash_password(password: &str) -> Result<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // TODO: bcrypt 等に置き換える
    let mut hasher = DefaultHasher::new();
    password.hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let expected = hash_password(password)?;
    if expected == hash {
        Ok(())
    } else {
        Err(anyhow::anyhow!("invalid credentials"))
    }
}
