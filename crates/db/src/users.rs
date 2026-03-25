use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
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
        password_hash
    )
    .execute(pool)
    .await?;

    crate::devices::create(pool, &user_id, &device_id).await?;
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
    crate::devices::create(pool, &user_id, &device_id).await?;
    let access_token = crate::access_tokens::create(pool, &user_id, &device_id).await?;

    Ok((user_id, access_token, device_id))
}

pub async fn change_password(
    pool: &MySqlPool,
    user_id: &str,
    old_password: &str,
    new_password: &str,
) -> Result<()> {
    let row = sqlx::query!("SELECT password_hash FROM users WHERE user_id = ?", user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user not found"))?;

    verify_password(old_password, &row.password_hash)?;

    let new_hash = hash_password(new_password)?;
    sqlx::query!(
        "UPDATE users SET password_hash = ? WHERE user_id = ?",
        new_hash,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// パスワードが正しいか検証する（変更は行わない）
/// user_id が DB に存在するか確認する。
pub async fn exists(pool: &MySqlPool, user_id: &str) -> Result<bool> {
    let row = sqlx::query("SELECT 1 FROM users WHERE user_id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// アカウントを無効化する: password_hash を空文字列に設定してログイン不能にする。
/// アクセストークンは呼び出し元で revoke_all() すること。
pub async fn deactivate(pool: &MySqlPool, user_id: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = '' WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// ユーザーが管理者かどうかを確認する。
pub async fn is_admin(pool: &MySqlPool, user_id: &str) -> Result<bool> {
    let row = sqlx::query("SELECT admin FROM users WHERE user_id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    use sqlx::Row;
    Ok(row.map(|r| r.get::<i8, _>("admin") != 0).unwrap_or(false))
}

/// 全ユーザーの一覧を返す（管理者向け）。
pub async fn list_all(pool: &MySqlPool) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT user_id, display_name, avatar_url, created_at, deactivated, admin \
         FROM users ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let created_at: chrono::NaiveDateTime = r.get("created_at");
            serde_json::json!({
                "user_id": r.get::<String, _>("user_id"),
                "display_name": r.get::<Option<String>, _>("display_name"),
                "avatar_url": r.get::<Option<String>, _>("avatar_url"),
                "creation_ts": created_at.and_utc().timestamp_millis(),
                "deactivated": r.get::<i8, _>("deactivated") != 0,
                "admin": r.get::<i8, _>("admin") != 0,
            })
        })
        .collect())
}

/// 管理者によるユーザー無効化（deactivated フラグを立て、全トークン・デバイスを削除）。
pub async fn admin_deactivate(pool: &MySqlPool, user_id: &str) -> Result<()> {
    sqlx::query("UPDATE users SET deactivated = 1, password_hash = '' WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    // 全アクセストークンを削除してログアウト状態にする
    sqlx::query("DELETE FROM access_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn verify(pool: &MySqlPool, user_id: &str, password: &str) -> Result<bool> {
    let row = sqlx::query!("SELECT password_hash FROM users WHERE user_id = ?", user_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => Ok(verify_password(password, &r.password_hash).is_ok()),
        None => Ok(false),
    }
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hash error: {}", e))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid hash: {}", e))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("invalid credentials"))
}
