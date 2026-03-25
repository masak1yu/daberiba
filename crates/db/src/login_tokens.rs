use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

/// ログイントークンを生成して保存する。有効期限は発行から 120 秒。
pub async fn create(pool: &MySqlPool, user_id: &str) -> Result<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = chrono::Utc::now().timestamp_millis() + 120_000;

    sqlx::query("INSERT INTO login_tokens (token, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(token)
}

/// トークンを検証して消費する。成功時は user_id を返す。
/// 未使用・期限内でなければ None を返す。
pub async fn consume(pool: &MySqlPool, token: &str) -> Result<Option<String>> {
    use sqlx::Row;

    let now_ms = chrono::Utc::now().timestamp_millis();

    let row = sqlx::query(
        "SELECT user_id FROM login_tokens WHERE token = ? AND used = 0 AND expires_at > ?",
    )
    .bind(token)
    .bind(now_ms)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let user_id: String = row.get("user_id");

    // 使用済みフラグを立てる（シングルユース）
    sqlx::query("UPDATE login_tokens SET used = 1 WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;

    Ok(Some(user_id))
}

/// 期限切れトークンを削除する（定期クリーンアップ用）
pub async fn purge_expired(pool: &MySqlPool) -> Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query("DELETE FROM login_tokens WHERE expires_at <= ?")
        .bind(now_ms)
        .execute(pool)
        .await?;
    Ok(())
}
