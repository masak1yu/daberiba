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
    let device_id = Uuid::new_v4().simple().to_string().to_uppercase();

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

    use sqlx::Row;
    let row = sqlx::query("SELECT password_hash, deactivated FROM users WHERE user_id = ?")
        .bind(&user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid credentials"))?;

    if row.get::<i8, _>("deactivated") != 0 {
        return Err(anyhow::anyhow!("invalid credentials"));
    }

    verify_password(password, &row.get::<String, _>("password_hash"))?;

    let device_id = Uuid::new_v4().simple().to_string().to_uppercase();
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

/// アカウントを無効化する: deactivated を 1 に設定し password_hash を空文字列にしてログイン不能にする。
/// アクセストークンは呼び出し元で revoke_all() すること。
pub async fn deactivate(pool: &MySqlPool, user_id: &str) -> Result<()> {
    sqlx::query("UPDATE users SET deactivated = 1, password_hash = '' WHERE user_id = ?")
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

/// ユーザー詳細を 1 件取得する（管理者向け）。
pub async fn get_by_id(pool: &MySqlPool, user_id: &str) -> Result<Option<serde_json::Value>> {
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT user_id, display_name, avatar_url, created_at, deactivated, admin \
         FROM users WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| {
        let created_at: chrono::NaiveDateTime = r.get("created_at");
        serde_json::json!({
            "user_id": r.get::<String, _>("user_id"),
            "display_name": r.get::<Option<String>, _>("display_name"),
            "avatar_url": r.get::<Option<String>, _>("avatar_url"),
            "creation_ts": created_at.and_utc().timestamp_millis(),
            "deactivated": r.get::<i8, _>("deactivated") != 0,
            "admin": r.get::<i8, _>("admin") != 0,
        })
    }))
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

/// ユーザーの管理者フラグを設定する。
pub async fn set_admin(pool: &MySqlPool, user_id: &str, admin: bool) -> Result<()> {
    sqlx::query("UPDATE users SET admin = ? WHERE user_id = ?")
        .bind(admin as i8)
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

/// ユーザーディレクトリ検索結果。
pub struct UserSearchResult {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// ユーザーディレクトリを検索する。
///
/// `term` で user_id または display_name を部分一致検索する。
/// `limit` 件まで返す（デフォルト 10、最大 50）。
/// 非アクティブユーザーは除外する。
pub async fn search_directory(
    pool: &MySqlPool,
    term: &str,
    limit: u64,
) -> Result<Vec<UserSearchResult>> {
    use sqlx::Row;

    let pattern = format!("%{}%", term);
    let rows = sqlx::query(
        "SELECT u.user_id, p.display_name, p.avatar_url \
         FROM users u \
         LEFT JOIN profiles p ON p.user_id = u.user_id \
         WHERE u.deactivated = 0 \
           AND (u.user_id LIKE ? OR p.display_name LIKE ?) \
         ORDER BY p.display_name ASC, u.user_id ASC \
         LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserSearchResult {
            user_id: r.get("user_id"),
            display_name: r.get("display_name"),
            avatar_url: r.get("avatar_url"),
        })
        .collect())
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid hash: {}", e))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("invalid credentials"))
}
