use anyhow::Result;
use sqlx::{MySqlPool, Row};

pub async fn get_all(pool: &MySqlPool, room_id: &str) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"SELECT e.event_id, e.sender, e.event_type, e.state_key, e.content, e.created_at
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ?"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let created_at: chrono::NaiveDateTime = r.get("created_at");
            let content_str: String = r.get("content");
            serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "state_key": r.get::<String, _>("state_key"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": created_at.and_utc().timestamp_millis(),
                "room_id": room_id,
            })
        })
        .collect())
}

pub async fn get_event(
    pool: &MySqlPool,
    room_id: &str,
    event_type: &str,
    state_key: &str,
) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query(
        r#"SELECT e.content
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ? AND rs.event_type = ? AND rs.state_key = ?"#,
    )
    .bind(room_id)
    .bind(event_type)
    .bind(state_key)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let content_str: String = r.get("content");
        serde_json::from_str::<serde_json::Value>(&content_str).unwrap_or_default()
    }))
}
