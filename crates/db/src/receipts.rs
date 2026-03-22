use anyhow::Result;
use sqlx::MySqlPool;

/// receipt を upsert（受信確認を記録）
pub async fn upsert(
    pool: &MySqlPool,
    room_id: &str,
    user_id: &str,
    receipt_type: &str,
    event_id: &str,
) -> Result<()> {
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"INSERT INTO receipts (room_id, user_id, receipt_type, event_id, ts)
           VALUES (?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE event_id = VALUES(event_id), ts = VALUES(ts)"#,
    )
    .bind(room_id)
    .bind(user_id)
    .bind(receipt_type)
    .bind(event_id)
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct Receipt {
    pub user_id: String,
    pub receipt_type: String,
    pub event_id: String,
    pub ts: i64,
}

/// room のすべての receipt を取得（sync 用）
pub async fn get_for_room(pool: &MySqlPool, room_id: &str) -> Result<Vec<Receipt>> {
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT user_id, receipt_type, event_id, ts FROM receipts WHERE room_id = ?",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(user_id, receipt_type, event_id, ts)| Receipt {
            user_id,
            receipt_type,
            event_id,
            ts,
        })
        .collect())
}

/// receipt を m.receipt イベント形式に変換
/// {"$event_id": {"m.read": {"@user:server": {"ts": 12345}}}}
pub fn to_event(receipts: Vec<Receipt>) -> serde_json::Value {
    let mut content = serde_json::Map::new();
    for r in receipts {
        let entry = content
            .entry(r.event_id.clone())
            .or_insert_with(|| serde_json::json!({}));
        let receipt_map = entry.as_object_mut().unwrap();
        let type_entry = receipt_map
            .entry(r.receipt_type.clone())
            .or_insert_with(|| serde_json::json!({}));
        let users = type_entry.as_object_mut().unwrap();
        users.insert(r.user_id.clone(), serde_json::json!({ "ts": r.ts }));
    }
    serde_json::json!({
        "type": "m.receipt",
        "content": content,
    })
}
