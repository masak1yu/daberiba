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
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let event_id = format!(
        "${}:{}",
        Uuid::new_v4().to_string().replace('-', ""),
        server_name
    );
    let content_str = serde_json::to_string(content)?;

    sqlx::query!(
        r#"INSERT INTO events (event_id, room_id, sender, event_type, state_key, content)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        event_id,
        room_id,
        sender,
        event_type,
        state_key,
        content_str
    )
    .execute(pool)
    .await?;

    if let Some(sk) = state_key {
        sqlx::query!(
            r#"INSERT INTO room_state (room_id, event_type, state_key, event_id)
               VALUES (?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE event_id = VALUES(event_id)"#,
            room_id,
            event_type,
            sk,
            event_id
        )
        .execute(pool)
        .await?;
    }

    Ok(event_id)
}

/// federation PDU のメタデータ
pub struct PduMeta<'a> {
    pub event_id: &'a str,
    pub room_id: &'a str,
    pub sender: &'a str,
    pub event_type: &'a str,
    pub state_key: Option<&'a str>,
    pub content: &'a serde_json::Value,
    pub origin_server_ts: i64,
}

/// federation PDU を受信して保存する。
///
/// `event_id` は PDU に含まれるオリジナルの event_id を使用する。
/// 既に同じ event_id が存在する場合は何もしない（べき等）。
/// state_key が Some の場合は状態解決ルールに基づいて room_state を更新する。
pub async fn store_pdu(pool: &MySqlPool, pdu: &PduMeta<'_>) -> Result<()> {
    let event_id = pdu.event_id;
    let room_id = pdu.room_id;
    let sender = pdu.sender;
    let event_type = pdu.event_type;
    let state_key = pdu.state_key;
    let origin_server_ts = pdu.origin_server_ts;
    let content_str = serde_json::to_string(pdu.content)?;

    // 重複受信は無視（同じ event_id を 2 度処理しない）
    let affected = sqlx::query(
        r#"INSERT IGNORE INTO events
           (event_id, room_id, sender, event_type, state_key, content, origin_server_ts)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(event_id)
    .bind(room_id)
    .bind(sender)
    .bind(event_type)
    .bind(state_key)
    .bind(&content_str)
    .bind(origin_server_ts)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        // 重複 — 何もしない
        return Ok(());
    }

    if let Some(sk) = state_key {
        // 状態解決: 既存の state event より origin_server_ts が新しい場合のみ更新
        // タイムスタンプが同一の場合は event_id の辞書順で小さい方（先着）を優先
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
        .bind(room_id)
        .bind(event_type)
        .bind(sk)
        .bind(event_id)
        .bind(origin_server_ts)
        .bind(origin_server_ts)
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
