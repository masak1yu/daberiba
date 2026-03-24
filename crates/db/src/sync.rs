use anyhow::Result;
use sqlx::{MySqlPool, Row as _};

/// /sync の実装
/// next_batch / since は stream_ordering（u64 文字列）
/// timeline_limit: フィルターで指定された timeline のイベント上限（デフォルト 50）
pub async fn sync(
    pool: &MySqlPool,
    user_id: &str,
    since: Option<&str>,
    timeline_limit: u32,
) -> Result<serde_json::Value> {
    let since_ordering: u64 = since.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let is_initial = since_ordering == 0;

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

        // limit+1 件取得して limit 件超なら limited = true
        let fetch_limit = (timeline_limit as i64) + 1;
        let mut event_rows = sqlx::query(
            "SELECT event_id, sender, event_type, state_key, content, created_at, stream_ordering \
             FROM events \
             WHERE room_id = ? AND stream_ordering > ? \
             ORDER BY stream_ordering ASC \
             LIMIT ?",
        )
        .bind(room_id)
        .bind(since_ordering)
        .bind(fetch_limit)
        .fetch_all(pool)
        .await?;

        let limited = event_rows.len() > timeline_limit as usize;
        if limited {
            event_rows.truncate(timeline_limit as usize);
        }

        if let Some(last) = event_rows.last() {
            let ord: u64 = last.get("stream_ordering");
            if ord > latest_ordering {
                latest_ordering = ord;
            }
        }

        // prev_batch: クライアントは GET /messages?dir=b&from=prev_batch で遡れる
        let prev_batch = if limited {
            event_rows
                .first()
                .map(|e| crate::events::ordering_to_token(e.get("stream_ordering")))
                .unwrap_or_else(|| crate::events::ordering_to_token(since_ordering))
        } else {
            crate::events::ordering_to_token(since_ordering)
        };

        let timeline_events: Vec<serde_json::Value> = event_rows
            .iter()
            .map(|e| {
                let content_str: String = e.get("content");
                serde_json::json!({
                    "event_id": e.get::<String, _>("event_id"),
                    "sender": e.get::<String, _>("sender"),
                    "type": e.get::<String, _>("event_type"),
                    "state_key": e.get::<Option<String>, _>("state_key"),
                    "content": serde_json::from_str::<serde_json::Value>(&content_str)
                        .unwrap_or_default(),
                    "origin_server_ts": e.get::<chrono::NaiveDateTime, _>("created_at")
                        .and_utc()
                        .timestamp_millis(),
                    "room_id": room_id,
                })
            })
            .collect();

        // state イベント取得:
        //  - 初回 sync: room_state から現在のスナップショット全体
        //  - 増分 limited: gap（since ~ timeline 先頭）の state 変更
        //  - 増分 non-limited: timeline に含まれるため空
        let state_events: Vec<serde_json::Value> = if is_initial {
            let rows = sqlx::query(
                r#"SELECT e.event_id, e.sender, e.event_type, rs.state_key, e.content, e.created_at
                   FROM room_state rs
                   JOIN events e ON e.event_id = rs.event_id
                   WHERE rs.room_id = ?"#,
            )
            .bind(room_id)
            .fetch_all(pool)
            .await?;

            rows.iter()
                .map(|r| {
                    let content_str: String = r.get("content");
                    serde_json::json!({
                        "event_id": r.get::<String, _>("event_id"),
                        "sender": r.get::<String, _>("sender"),
                        "type": r.get::<String, _>("event_type"),
                        "state_key": r.get::<String, _>("state_key"),
                        "content": serde_json::from_str::<serde_json::Value>(&content_str)
                            .unwrap_or_default(),
                        "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                            .and_utc()
                            .timestamp_millis(),
                        "room_id": room_id,
                    })
                })
                .collect()
        } else if limited {
            // gap 内（since_ordering < ord < timeline 先頭）の state イベントを返す
            let gap_end: u64 = event_rows
                .first()
                .map(|e| e.get("stream_ordering"))
                .unwrap_or(since_ordering);
            let rows = sqlx::query(
                r#"SELECT event_id, sender, event_type, state_key, content, created_at
                   FROM events
                   WHERE room_id = ?
                     AND state_key IS NOT NULL
                     AND stream_ordering > ?
                     AND stream_ordering < ?
                   ORDER BY stream_ordering ASC"#,
            )
            .bind(room_id)
            .bind(since_ordering)
            .bind(gap_end)
            .fetch_all(pool)
            .await?;

            rows.iter()
                .map(|r| {
                    let content_str: String = r.get("content");
                    serde_json::json!({
                        "event_id": r.get::<String, _>("event_id"),
                        "sender": r.get::<String, _>("sender"),
                        "type": r.get::<String, _>("event_type"),
                        "state_key": r.get::<Option<String>, _>("state_key"),
                        "content": serde_json::from_str::<serde_json::Value>(&content_str)
                            .unwrap_or_default(),
                        "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                            .and_utc()
                            .timestamp_millis(),
                        "room_id": room_id,
                    })
                })
                .collect()
        } else {
            vec![]
        };

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
                    "limited": limited,
                    "prev_batch": prev_batch,
                },
                "state": { "events": state_events },
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

    // rooms.leave: 増分 sync 時、since_ordering より後に leave になったルームを返す
    let mut leave_map = serde_json::Map::new();
    if !is_initial {
        let leave_room_ids = crate::rooms::leave_rooms_since(pool, user_id, since_ordering).await?;
        for room_id in leave_room_ids {
            // leave イベント（m.room.member / state_key = user_id）を取得
            let leave_events: Vec<serde_json::Value> = sqlx::query(
                r#"SELECT event_id, sender, event_type, state_key, content, created_at
                   FROM events
                   WHERE room_id = ? AND event_type = 'm.room.member' AND state_key = ?
                     AND stream_ordering > ?
                     AND content LIKE '%"membership":"leave"%'
                   ORDER BY stream_ordering DESC LIMIT 1"#,
            )
            .bind(&room_id)
            .bind(user_id)
            .bind(since_ordering)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .iter()
            .map(|r| {
                use sqlx::Row;
                let content_str: String = r.get("content");
                serde_json::json!({
                    "event_id": r.get::<String, _>("event_id"),
                    "sender": r.get::<String, _>("sender"),
                    "type": r.get::<String, _>("event_type"),
                    "state_key": r.get::<Option<String>, _>("state_key"),
                    "content": serde_json::from_str::<serde_json::Value>(&content_str)
                        .unwrap_or_default(),
                    "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                        .and_utc()
                        .timestamp_millis(),
                    "room_id": room_id,
                })
            })
            .collect();

            leave_map.insert(
                room_id,
                serde_json::json!({
                    "timeline": { "events": leave_events, "limited": false },
                    "state": { "events": [] },
                }),
            );
        }
    }

    Ok(serde_json::json!({
        "next_batch": latest_ordering.to_string(),
        "rooms": {
            "join": join_map,
            "invite": invite_map,
            "leave": leave_map,
        },
        "presence": { "events": [] },
    }))
}
