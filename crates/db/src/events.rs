use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

pub async fn send(
    pool: &MySqlPool,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: Option<&str>,
    content: &serde_json::Value,
) -> Result<String> {
    let event_id = format!("${}:{}", Uuid::new_v4().to_string().replace('-', ""), {
        // server_name をevent_idに含めるため環境変数から取得
        std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string())
    });
    let content_str = serde_json::to_string(content)?;

    sqlx::query!(
        r#"
        INSERT INTO events (event_id, room_id, sender, event_type, state_key, content)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
        event_id,
        room_id,
        sender,
        event_type,
        state_key,
        content_str,
    )
    .execute(pool)
    .await?;

    // state event の場合は room_state を upsert
    if let Some(sk) = state_key {
        sqlx::query!(
            r#"
            INSERT INTO room_state (room_id, event_type, state_key, event_id)
            VALUES (?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE event_id = VALUES(event_id)
            "#,
            room_id,
            event_type,
            sk,
            event_id,
        )
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
    let rows = sqlx::query!(
        r#"
        SELECT event_id, sender, event_type, content, created_at
        FROM events
        WHERE room_id = ? AND state_key IS NULL
        ORDER BY created_at DESC
        LIMIT ?
        "#,
        room_id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    let events = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "event_id": r.event_id,
                "sender": r.sender,
                "type": r.event_type,
                "content": serde_json::from_str::<serde_json::Value>(&r.content).unwrap_or_default(),
                "origin_server_ts": r.created_at.and_utc().timestamp_millis(),
                "room_id": room_id,
            })
        })
        .collect();

    Ok(events)
}
