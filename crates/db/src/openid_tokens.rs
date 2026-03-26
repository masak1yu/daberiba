use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

/// OpenID トークンを発行して DB に保存。有効期限は 3600 秒。
pub async fn create(pool: &MySqlPool, user_id: &str) -> Result<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = chrono::Utc::now().timestamp_millis() + 3_600_000;
    sqlx::query("INSERT INTO openid_tokens (token, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(token)
}

/// トークンを検証して user_id を返す。有効期限切れまたは存在しない場合は None。
pub async fn verify(pool: &MySqlPool, token: &str) -> Result<Option<String>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let row: Option<(String,)> =
        sqlx::query_as("SELECT user_id FROM openid_tokens WHERE token = ? AND expires_at > ?")
            .bind(token)
            .bind(now_ms)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(uid,)| uid))
}

/// 期限切れトークンを削除（定期クリーンアップ用）
pub async fn purge_expired(pool: &MySqlPool) -> Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query("DELETE FROM openid_tokens WHERE expires_at <= ?")
        .bind(now_ms)
        .execute(pool)
        .await?;
    Ok(())
}
