use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::{MySqlPool, Row};

/// /sync の実装
/// next_batch / since は stream_ordering（u64 文字列）
pub async fn sync(
    pool: &MySqlPool,
    user_id: &str,
    since: Option<&str>,
) -> Result<serde_json::Value> {
    let since_ordering: i64 = since.and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);

    let rooms = sqlx::query(
        "SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut join_map = serde_json::Map::new();
    let mut latest_ordering: i64 = since_ordering;

    for room_row in &rooms {
        let room_id: String = room_row.get("room_id");

        let event_rows = sqlx::query(
            r#"SELECT event_id, sender, event_type, state_key, content, created_at, stream_ordering
               FROM events
               WHERE room_id = ? AND stream_ordering > ?
               ORDER BY stream_ordering ASC
               LIMIT 100"#,
        )
        .bind(&room_id)
        .bind(since_ordering)
        .fetch_all(pool)
        .await?;

        if let Some(last) = event_rows.last() {
            let ord: i64 = last.get::<u64, _>("stream_ordering") as i64;
            if ord > latest_ordering {
                latest_ordering = ord;
            }
        }

        let prev_batch = since_ordering.to_string();
        let timeline_events: Vec<serde_json::Value> = event_rows
            .iter()
            .map(|e| {
                let created_at: NaiveDateTime = e.get("created_at");
                let content_str: String = e.get("content");
                let state_key: Option<String> = e.get("state_key");
                serde_json::json!({
                    "event_id": e.get::<String, _>("event_id"),
                    "sender": e.get::<String, _>("sender"),
                    "type": e.get::<String, _>("event_type"),
                    "state_key": state_key,
                    "content": serde_json::from_str::<serde_json::Value>(&content_str)
                        .unwrap_or_default(),
                    "origin_server_ts": created_at.and_utc().timestamp_millis(),
                    "room_id": room_id,
                })
            })
            .collect();

        join_map.insert(
            room_id,
            serde_json::json!({
                "timeline": {
                    "events": timeline_events,
                    "limited": false,
                    "prev_batch": prev_batch,
                },
                "state": { "events": [] },
                "ephemeral": { "events": [] },
            }),
        );
    }

    Ok(serde_json::json!({
        "next_batch": latest_ordering.to_string(),
        "rooms": {
            "join": join_map,
            "invite": {},
            "leave": {},
        },
        "presence": { "events": [] },
    }))
}
