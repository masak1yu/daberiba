use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::{MySqlPool, Row};
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

    sqlx::query("INSERT INTO users (user_id, password_hash) VALUES (?, ?)")
        .bind(&user_id)
        .bind(&password_hash)
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

    let row = sqlx::query("SELECT password_hash FROM users WHERE user_id = ?")
        .bind(&user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid credentials"))?;

    let password_hash: String = row.get("password_hash");
    verify_password(password, &password_hash)?;

    let device_id = Uuid::new_v4().to_string().to_uppercase()[..8].to_string();
    let access_token = crate::access_tokens::create(pool, &user_id, &device_id).await?;

    Ok((user_id, access_token, device_id))
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hash error: {}", e))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("invalid hash: {}", e))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("invalid credentials"))
}
