use anyhow::Result;
use sqlx::MySqlPool;

pub struct ToDeviceMessage {
    pub id: u64,
    pub sender: String,
    pub event_type: String,
    pub content: String,
}

/// メッセージを送信（PUT /sendToDevice/{type}/{txnId}）
/// device_id = "*" で全デバイス宛て
pub async fn send(
    pool: &MySqlPool,
    sender: &str,
    recipient: &str,
    device_id: &str,
    event_type: &str,
    content: &str,
    txn_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO to_device_messages (sender, recipient, device_id, event_type, content, txn_id)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(sender)
    .bind(recipient)
    .bind(device_id)
    .bind(event_type)
    .bind(content)
    .bind(txn_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// ユーザー宛ての未配信メッセージを取得（/sync 用）
pub async fn get_pending(pool: &MySqlPool, user_id: &str) -> Result<Vec<ToDeviceMessage>> {
    let rows: Vec<(u64, String, String, String)> = sqlx::query_as(
        r#"SELECT id, sender, event_type, content
           FROM to_device_messages
           WHERE recipient = ?
           ORDER BY id ASC
           LIMIT 100"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, sender, event_type, content)| ToDeviceMessage {
            id,
            sender,
            event_type,
            content,
        })
        .collect())
}

/// 前回 sync で配信済みのメッセージを削除（acked_up_to 以下の id を削除）
pub async fn delete_acked(pool: &MySqlPool, user_id: &str, acked_up_to: u64) -> Result<()> {
    if acked_up_to == 0 {
        return Ok(());
    }
    sqlx::query("DELETE FROM to_device_messages WHERE recipient = ? AND id <= ?")
        .bind(user_id)
        .bind(acked_up_to)
        .execute(pool)
        .await?;
    Ok(())
}
