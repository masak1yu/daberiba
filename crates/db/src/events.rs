use anyhow::Result;
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

pub async fn send(
    pool: &MySqlPool,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: Option<&str>,
    content: &serde_json::Value,
) -> Result<String> {
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let event_id = format!(
        "${}:{}",
        Uuid::new_v4().to_string().replace('-', ""),
        server_name
    );
    let content_str = serde_json::to_string(content)?;

    sqlx::query(
        r#"INSERT INTO events (event_id, room_id, sender, event_type, state_key, content)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&event_id)
    .bind(room_id)
    .bind(sender)
    .bind(event_type)
    .bind(state_key)
    .bind(&content_str)
    .execute(pool)
    .await?;

    if let Some(sk) = state_key {
        sqlx::query(
            r#"INSERT INTO room_state (room_id, event_type, state_key, event_id)
               VALUES (?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE event_id = VALUES(event_id)"#,
        )
        .bind(room_id)
        .bind(event_type)
        .bind(sk)
        .bind(&event_id)
        .execute(pool)
        .await?;
    }

    Ok(event_id)
}

pub async fn get_messages(
    pool: &MySqlPool,
    room_id: &str,
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"SELECT event_id, sender, event_type, content, created_at
           FROM events
           WHERE room_id = ? AND state_key IS NULL
           ORDER BY created_at DESC
           LIMIT ?"#,
    )
    .bind(room_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let events = rows
        .into_iter()
        .map(|r| {
            let created_at: chrono::NaiveDateTime = r.get("created_at");
            let content_str: String = r.get("content");
            serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": created_at.and_utc().timestamp_millis(),
                "room_id": room_id,
            })
        })
        .collect();

    Ok(events)
}
