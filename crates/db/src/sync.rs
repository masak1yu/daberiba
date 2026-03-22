use anyhow::Result;
use sqlx::MySqlPool;

/// /sync の実装
/// next_batch / since は stream_ordering（u64 文字列）
pub async fn sync(
    pool: &MySqlPool,
    user_id: &str,
    since: Option<&str>,
) -> Result<serde_json::Value> {
    let since_ordering: u64 = since.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

    let rooms = sqlx::query!(
        "SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'",
        user_id
    )
    .fetch_all(pool)
    .await?;

    let mut join_map = serde_json::Map::new();
    let mut latest_ordering: u64 = since_ordering;

    for room_row in &rooms {
        let room_id = &room_row.room_id;

        let event_rows = sqlx::query!(
            r#"SELECT event_id, sender, event_type, state_key, content, created_at, stream_ordering
               FROM events
               WHERE room_id = ? AND stream_ordering > ?
               ORDER BY stream_ordering ASC
               LIMIT 100"#,
            room_id,
            since_ordering
        )
        .fetch_all(pool)
        .await?;

        if let Some(last) = event_rows.last() {
            let ord = last.stream_ordering;
            if ord > latest_ordering {
                latest_ordering = ord;
            }
        }

        let prev_batch = since_ordering.to_string();
        let timeline_events: Vec<serde_json::Value> = event_rows
            .iter()
            .map(|e| {
                let content_str = &e.content;
                serde_json::json!({
                    "event_id": e.event_id,
                    "sender": e.sender,
                    "type": e.event_type,
                    "state_key": e.state_key,
                    "content": serde_json::from_str::<serde_json::Value>(content_str)
                        .unwrap_or_default(),
                    "origin_server_ts": e.created_at.and_utc().timestamp_millis(),
                    "room_id": room_id,
                })
            })
            .collect();

        let unread = crate::unread::get_for_room(pool, room_id, user_id)
            .await
            .unwrap_or(crate::unread::UnreadCounts {
                notification_count: 0,
                highlight_count: 0,
            });

        join_map.insert(
            room_id.clone(),
            serde_json::json!({
                "timeline": {
                    "events": timeline_events,
                    "limited": false,
                    "prev_batch": prev_batch,
                },
                "state": { "events": [] },
                "ephemeral": { "events": [] },
                "unread_notifications": {
                    "notification_count": unread.notification_count,
                    "highlight_count": unread.highlight_count,
                },
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
