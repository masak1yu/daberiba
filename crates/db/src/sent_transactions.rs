use anyhow::Result;
use sqlx::MySqlPool;

/// 送信済みトランザクションを記録する。既存の場合は何もしない（INSERT IGNORE）。
pub async fn record(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    txn_id: &str,
    event_id: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT IGNORE INTO sent_transactions (user_id, device_id, txn_id, event_id) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(txn_id)
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 既存のトランザクションから event_id を取得する。未記録の場合は None。
pub async fn get_event_id(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    txn_id: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT event_id FROM sent_transactions WHERE user_id = ? AND device_id = ? AND txn_id = ?",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(txn_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| id))
}
