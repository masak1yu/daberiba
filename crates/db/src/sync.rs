use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::{MySqlPool, Row};

/// /sync の最小実装
/// next_batch / since はミリ秒UNIXタイムスタンプ（文字列）
pub async fn sync(
    pool: &MySqlPool,
    user_id: &str,
    since: Option<&str>,
) -> Result<serde_json::Value> {
    let since_dt: Option<NaiveDateTime> = since
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|ms| chrono::DateTime::from_timestamp_millis(ms))
        .map(|dt| dt.naive_utc());

    let rooms = sqlx::query(
        "SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut join_map = serde_json::Map::new();
    let mut latest_ts: Option<NaiveDateTime> = None;

    for room_row in &rooms {
        let room_id: String = room_row.get("room_id");

        let event_rows = if let Some(dt) = since_dt {
            sqlx::query(
                r#"SELECT event_id, sender, event_type, state_key, content, created_at
                   FROM events
                   WHERE room_id = ? AND created_at > ?
                   ORDER BY created_at ASC
                   LIMIT 100"#,
            )
            .bind(&room_id)
            .bind(dt)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"SELECT event_id, sender, event_type, state_key, content, created_at
                   FROM events
                   WHERE room_id = ?
                   ORDER BY created_at ASC
                   LIMIT 100"#,
            )
            .bind(&room_id)
            .fetch_all(pool)
            .await?
        };

        if let Some(last) = event_rows.last() {
            let ts: NaiveDateTime = last.get("created_at");
            if latest_ts.map_or(true, |t| ts > t) {
                latest_ts = Some(ts);
            }
        }

        let prev_batch = since.unwrap_or("0").to_string();
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

    let next_batch = latest_ts
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
        .to_string();

    Ok(serde_json::json!({
        "next_batch": next_batch,
        "rooms": {
            "join": join_map,
            "invite": {},
            "leave": {},
        },
        "presence": { "events": [] },
    }))
}
