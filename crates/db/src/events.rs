use anyhow::Result;
use sqlx::MySqlPool;

/// ローカルイベントのメタデータ。
///
/// event_id と origin_server_ts は呼び出し元が計算して渡す（room v3+ ハッシュベース推奨）。
pub struct LocalEvent<'a> {
    pub event_id: &'a str,
    pub room_id: &'a str,
    pub sender: &'a str,
    pub event_type: &'a str,
    pub state_key: Option<&'a str>,
    pub content: &'a serde_json::Value,
    pub origin_server_ts: i64,
}

/// ルームの現在の "tip" 情報を返す。
///
/// 戻り値: (next_depth, prev_event_ids)
/// - next_depth: 次に作成するイベントに使う depth（既存最大 + 1、初回は 1）
/// - prev_event_ids: 直前イベントの event_id リスト（stream_ordering 最大のもの）
pub async fn get_room_tip(pool: &MySqlPool, room_id: &str) -> Result<(i64, Vec<String>)> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT event_id, depth FROM events WHERE room_id = ? ORDER BY stream_ordering DESC LIMIT 1",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some(r) => {
            let depth: i64 = r.get("depth");
            let event_id: String = r.get("event_id");
            (depth + 1, vec![event_id])
        }
        None => (1, vec![]),
    })
}

/// ローカルイベントを保存する。
///
/// depth はルームの現在の最大 depth + 1 として自動計算する。
/// 戻り値: 保存したイベントの depth と prev_event_ids（federation PDU 構築に使用）。
pub async fn send(pool: &MySqlPool, ev: &LocalEvent<'_>) -> Result<(i64, Vec<String>)> {
    let (depth, prev_event_ids) = get_room_tip(pool, ev.room_id).await?;
    let content_str = serde_json::to_string(ev.content)?;
    let prev_events_str = serde_json::to_string(&prev_event_ids)?;

    sqlx::query(
        r#"INSERT INTO events (event_id, room_id, sender, event_type, state_key, content, origin_server_ts, depth, prev_events)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(ev.event_id)
    .bind(ev.room_id)
    .bind(ev.sender)
    .bind(ev.event_type)
    .bind(ev.state_key)
    .bind(&content_str)
    .bind(ev.origin_server_ts)
    .bind(depth)
    .bind(&prev_events_str)
    .execute(pool)
    .await?;

    if let Some(sk) = ev.state_key {
        sqlx::query(
            r#"INSERT INTO room_state (room_id, event_type, state_key, event_id)
               VALUES (?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE event_id = VALUES(event_id)"#,
        )
        .bind(ev.room_id)
        .bind(ev.event_type)
        .bind(sk)
        .bind(ev.event_id)
        .execute(pool)
        .await?;
    }

    Ok((depth, prev_event_ids))
}

/// federation PDU のメタデータ
pub struct PduMeta<'a> {
    pub event_id: &'a str,
    pub room_id: &'a str,
    pub sender: &'a str,
    pub event_type: &'a str,
    pub state_key: Option<&'a str>,
    pub content: &'a serde_json::Value,
    /// auth_events フィールド（JSON 配列）。None の場合は保存しない。
    pub auth_events: Option<&'a serde_json::Value>,
    /// prev_events フィールド（JSON 配列）。None の場合は保存しない。
    pub prev_events: Option<&'a serde_json::Value>,
    pub origin_server_ts: i64,
    /// DAG の深さ。PDU に含まれていない場合は 0。
    pub depth: i64,
}

/// federation PDU を受信して保存する。
///
/// `event_id` は PDU に含まれるオリジナルの event_id を使用する。
/// 既に同じ event_id が存在する場合は何もしない（べき等）。
/// state_key が Some の場合は状態解決ルールに基づいて room_state を更新する。
pub async fn store_pdu(pool: &MySqlPool, pdu: &PduMeta<'_>) -> Result<()> {
    let content_str = serde_json::to_string(pdu.content)?;
    let auth_events_str = pdu.auth_events.map(serde_json::to_string).transpose()?;
    let prev_events_str = pdu.prev_events.map(serde_json::to_string).transpose()?;

    let affected = sqlx::query(
        r#"INSERT IGNORE INTO events
           (event_id, room_id, sender, event_type, state_key, content, auth_events, prev_events, origin_server_ts, depth)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(pdu.event_id)
    .bind(pdu.room_id)
    .bind(pdu.sender)
    .bind(pdu.event_type)
    .bind(pdu.state_key)
    .bind(&content_str)
    .bind(auth_events_str.as_deref())
    .bind(prev_events_str.as_deref())
    .bind(pdu.origin_server_ts)
    .bind(pdu.depth)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Ok(());
    }

    if let Some(sk) = pdu.state_key {
        // 状態解決: origin_server_ts が新しい方を採用。同一 ts は event_id 辞書順で小さい方を優先
        sqlx::query(
            r#"INSERT INTO room_state (room_id, event_type, state_key, event_id)
               VALUES (?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE
                 event_id = IF(
                   (SELECT origin_server_ts FROM events WHERE event_id = room_state.event_id) < ?,
                   VALUES(event_id),
                   IF(
                     (SELECT origin_server_ts FROM events WHERE event_id = room_state.event_id) = ?
                       AND room_state.event_id > VALUES(event_id),
                     VALUES(event_id),
                     room_state.event_id
                   )
                 )"#,
        )
        .bind(pdu.room_id)
        .bind(pdu.event_type)
        .bind(sk)
        .bind(pdu.event_id)
        .bind(pdu.origin_server_ts)
        .bind(pdu.origin_server_ts)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// event_id でイベントを取得する。見つからない場合は None。
pub async fn get_by_id(pool: &MySqlPool, event_id: &str) -> Result<Option<serde_json::Value>> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT event_id, room_id, sender, event_type, state_key, content, created_at \
         FROM events WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let content_str: String = r.get("content");
        let state_key: Option<String> = r.get("state_key");
        let mut event = serde_json::json!({
            "event_id": r.get::<String, _>("event_id"),
            "room_id": r.get::<String, _>("room_id"),
            "sender": r.get::<String, _>("sender"),
            "type": r.get::<String, _>("event_type"),
            "content": serde_json::from_str::<serde_json::Value>(&content_str)
                .unwrap_or_default(),
            "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                .and_utc()
                .timestamp_millis(),
        });
        if let Some(sk) = state_key {
            event["state_key"] = serde_json::Value::String(sk);
        }
        event
    }))
}

/// backfill 用: 指定 event_id より古いイベントを最大 limit 件取得する。
///
/// `v` クエリパラメータで指定された event_id を起点として、それより古い
/// stream_ordering のイベントを降順で返す。
pub async fn get_backfill(
    pool: &MySqlPool,
    room_id: &str,
    from_event_ids: &[String],
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    // from_event_ids の最小 stream_ordering を起点とする
    let limit = limit.min(100) as i64;

    // from_event_ids がある場合、それらの stream_ordering を取得して最小値を起点にする
    let start_ordering: Option<u64> = if from_event_ids.is_empty() {
        None
    } else {
        // IN 句は動的に構築
        let placeholders = from_event_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT MIN(stream_ordering) FROM events WHERE event_id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&sql);
        for id in from_event_ids {
            q = q.bind(id);
        }
        q.fetch_one(pool).await?.get::<Option<u64>, _>(0)
    };

    let rows: Vec<sqlx::mysql::MySqlRow> = match start_ordering {
        Some(ord) => {
            sqlx::query(
                "SELECT event_id, room_id, sender, event_type, state_key, content, \
                 origin_server_ts, created_at, stream_ordering \
                 FROM events \
                 WHERE room_id = ? AND stream_ordering < ? \
                 ORDER BY stream_ordering DESC LIMIT ?",
            )
            .bind(room_id)
            .bind(ord)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT event_id, room_id, sender, event_type, state_key, content, \
                 origin_server_ts, created_at, stream_ordering \
                 FROM events \
                 WHERE room_id = ? \
                 ORDER BY stream_ordering DESC LIMIT ?",
            )
            .bind(room_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    let events = rows
        .iter()
        .map(|r| {
            let content_str: String = r.get("content");
            let state_key: Option<String> = r.get("state_key");
            let ts: i64 = r
                .get::<Option<i64>, _>("origin_server_ts")
                .unwrap_or_else(|| {
                    r.get::<chrono::NaiveDateTime, _>("created_at")
                        .and_utc()
                        .timestamp_millis()
                });
            let mut event = serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "room_id": r.get::<String, _>("room_id"),
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": ts,
            });
            if let Some(sk) = state_key {
                event["state_key"] = serde_json::Value::String(sk);
            }
            event
        })
        .collect();

    Ok(events)
}

/// ページネーション結果。
/// `start` / `end` は `s{stream_ordering}` 形式のトークン。
/// `end` が `None` の場合はこれ以上イベントがない。
pub struct MessagePage {
    pub events: Vec<serde_json::Value>,
    pub start: String,
    pub end: Option<String>,
}

pub fn ordering_to_token(n: u64) -> String {
    format!("s{}", n)
}

/// `s{ordering}` 形式のトークンを u64 に変換する。失敗時は None。
pub fn parse_token(token: &str) -> Option<u64> {
    token.strip_prefix('s')?.parse().ok()
}

/// `/messages` 用ページネーションクエリ。
///
/// - `from`: カーソル（stream_ordering）。None の場合は先端から取得。
/// - `dir`: "b"（新しい順）または "f"（古い順）。デフォルト "b"。
/// - `limit`: 取得件数上限。
pub async fn get_messages(
    pool: &MySqlPool,
    room_id: &str,
    from: Option<u64>,
    dir: &str,
    limit: u32,
) -> Result<MessagePage> {
    use sqlx::Row;

    let fetch_limit = (limit as i64) + 1;
    let backward = dir != "f";
    let order_clause = if backward { "DESC" } else { "ASC" };
    let cmp_op = if backward { "<" } else { ">" };

    let rows: Vec<sqlx::mysql::MySqlRow> = match from {
        Some(n) => {
            sqlx::query(&format!(
                "SELECT event_id, sender, event_type, content, created_at, stream_ordering \
                 FROM events \
                 WHERE room_id = ? AND state_key IS NULL AND stream_ordering {} ? \
                 ORDER BY stream_ordering {} LIMIT ?",
                cmp_op, order_clause
            ))
            .bind(room_id)
            .bind(n)
            .bind(fetch_limit)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(&format!(
                "SELECT event_id, sender, event_type, content, created_at, stream_ordering \
                 FROM events \
                 WHERE room_id = ? AND state_key IS NULL \
                 ORDER BY stream_ordering {} LIMIT ?",
                order_clause
            ))
            .bind(room_id)
            .bind(fetch_limit)
            .fetch_all(pool)
            .await?
        }
    };

    let has_more = rows.len() > limit as usize;
    let rows = if has_more {
        &rows[..limit as usize]
    } else {
        &rows[..]
    };

    let start = rows
        .first()
        .map(|r| ordering_to_token(r.get("stream_ordering")))
        .unwrap_or_default();

    let end = if has_more {
        rows.last()
            .map(|r| ordering_to_token(r.get("stream_ordering")))
    } else {
        None
    };

    let events = rows
        .iter()
        .map(|r| {
            let content_str: String = r.get("content");
            serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                    .and_utc()
                    .timestamp_millis(),
                "room_id": room_id,
            })
        })
        .collect();

    Ok(MessagePage { events, start, end })
}
