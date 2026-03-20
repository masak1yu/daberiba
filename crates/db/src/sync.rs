use anyhow::Result;
use sqlx::MySqlPool;

/// /sync の最小実装
/// since: prev_batch トークン（今はevent_id順序で代替）
pub async fn sync(
    pool: &MySqlPool,
    user_id: &str,
    since: Option<&str>,
) -> Result<serde_json::Value> {
    // 参加中のルーム一覧
    let rooms = sqlx::query!(
        "SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'",
        user_id,
    )
    .fetch_all(pool)
    .await?;

    let mut join_map = serde_json::Map::new();

    for room in rooms {
        let room_id = &room.room_id;

        // タイムライン取得（since 以降）
        let events = if let Some(s) = since {
            sqlx::query!(
                r#"
                SELECT event_id, sender, event_type, state_key, content, created_at
                FROM events
                WHERE room_id = ? AND event_id > ?
                ORDER BY created_at ASC
                LIMIT 100
                "#,
                room_id,
                s,
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query!(
                r#"
                SELECT event_id, sender, event_type, state_key, content, created_at
                FROM events
                WHERE room_id = ?
                ORDER BY created_at ASC
                LIMIT 100
                "#,
                room_id,
            )
            .fetch_all(pool)
            .await?
        };

        let timeline_events: Vec<serde_json::Value> = events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "event_id": e.event_id,
                    "sender": e.sender,
                    "type": e.event_type,
                    "state_key": e.state_key,
                    "content": serde_json::from_str::<serde_json::Value>(&e.content).unwrap_or_default(),
                    "origin_server_ts": e.created_at.and_utc().timestamp_millis(),
                    "room_id": room_id,
                })
            })
            .collect();

        let prev_batch = since.unwrap_or("").to_string();
        let next_batch = events.last().map(|e| e.event_id.clone()).unwrap_or_default();

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
            }),
        );

        let _ = next_batch; // 後で next_batch に使う
    }

    let next_batch = chrono::Utc::now().timestamp_millis().to_string();

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
