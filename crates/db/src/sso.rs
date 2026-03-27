use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

/// SSO state を生成してDBに保存する。有効期限は 5 分。
pub async fn create_state(pool: &MySqlPool, redirect_url: &str) -> Result<String> {
    let state = Uuid::new_v4().to_string().replace('-', "");
    let expires_at = chrono::Utc::now().timestamp_millis() + 300_000; // 5 分

    sqlx::query("INSERT INTO sso_states (state, redirect_url, expires_at) VALUES (?, ?, ?)")
        .bind(&state)
        .bind(redirect_url)
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(state)
}

/// state を消費して redirect_url を返す。期限切れ・未存在の場合は None。
pub async fn consume_state(pool: &MySqlPool, state: &str) -> Result<Option<String>> {
    use sqlx::Row;

    let now_ms = chrono::Utc::now().timestamp_millis();

    let row = sqlx::query("SELECT redirect_url FROM sso_states WHERE state = ? AND expires_at > ?")
        .bind(state)
        .bind(now_ms)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let redirect_url: String = row.get("redirect_url");

    // 使い捨て — 即削除
    sqlx::query("DELETE FROM sso_states WHERE state = ?")
        .bind(state)
        .execute(pool)
        .await?;

    Ok(Some(redirect_url))
}

/// OIDC sub からマッピング済み user_id を返す。未登録の場合は None。
pub async fn find_user_by_sub(pool: &MySqlPool, sub: &str) -> Result<Option<String>> {
    use sqlx::Row;

    let row = sqlx::query("SELECT user_id FROM sso_accounts WHERE sub = ?")
        .bind(sub)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.get("user_id")))
}

/// OIDC sub と Matrix user_id のマッピングを登録する。
pub async fn link_account(pool: &MySqlPool, sub: &str, user_id: &str) -> Result<()> {
    sqlx::query("INSERT IGNORE INTO sso_accounts (sub, user_id) VALUES (?, ?)")
        .bind(sub)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 期限切れ state を削除する（定期クリーンアップ用）。
pub async fn purge_expired_states(pool: &MySqlPool) -> Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query("DELETE FROM sso_states WHERE expires_at <= ?")
        .bind(now_ms)
        .execute(pool)
        .await?;
    Ok(())
}
