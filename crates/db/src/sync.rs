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

    // rooms.invite: membership = 'invite' なルームを stripped state で返す
    let invited = crate::rooms::invited_rooms(pool, user_id).await?;
    let mut invite_map = serde_json::Map::new();
    for inv in invited {
        let mut invite_state_events: Vec<serde_json::Value> = Vec::new();

        // ルーム名があれば追加
        if let Ok(Some(content)) =
            crate::room_state::get_event(pool, &inv.room_id, "m.room.name", "").await
        {
            invite_state_events.push(serde_json::json!({
                "type": "m.room.name",
                "state_key": "",
                "content": content,
                "sender": inv.invited_by.as_deref().unwrap_or(""),
            }));
        }

        // 招待者の m.room.member
        if let Some(ref inviter) = inv.invited_by {
            invite_state_events.push(serde_json::json!({
                "type": "m.room.member",
                "state_key": inviter,
                "content": { "membership": "join" },
                "sender": inviter,
            }));
        }

        // 被招待者の m.room.member
        invite_state_events.push(serde_json::json!({
            "type": "m.room.member",
            "state_key": user_id,
            "content": { "membership": "invite" },
            "sender": inv.invited_by.as_deref().unwrap_or(""),
        }));

        invite_map.insert(
            inv.room_id,
            serde_json::json!({ "invite_state": { "events": invite_state_events } }),
        );
    }

    Ok(serde_json::json!({
        "next_batch": latest_ordering.to_string(),
        "rooms": {
            "join": join_map,
            "invite": invite_map,
            "leave": {},
        },
        "presence": { "events": [] },
    }))
}
