use anyhow::Result;
use sqlx::MySqlPool;

/// ローカルイベントのメタデータ。
///
/// event_id・depth・prev_events・origin_server_ts はすべて呼び出し元が計算して渡す。
/// get_room_tip() で depth/prev_events を取得してから PDU ハッシュを計算し、
/// その値をそのままここに設定することで event_id と保存済みフィールドが一致する。
pub struct LocalEvent<'a> {
    pub event_id: &'a str,
    pub room_id: &'a str,
    pub sender: &'a str,
    pub event_type: &'a str,
    pub state_key: Option<&'a str>,
    pub content: &'a serde_json::Value,
    pub origin_server_ts: i64,
    pub depth: i64,
    pub prev_events: &'a [String],
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
/// depth と prev_events は呼び出し元が get_room_tip() で取得して渡す。
/// これにより event_id（呼び出し元で PDU ハッシュから計算）と保存フィールドが一致する。
pub async fn send(pool: &MySqlPool, ev: &LocalEvent<'_>) -> Result<()> {
    let content_str = serde_json::to_string(ev.content)?;
    let prev_events_str = serde_json::to_string(ev.prev_events)?;

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
    .bind(ev.depth)
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

    // m.relates_to が含まれる場合は event_relations に記録する
    if let (Some(rel_type), Some(rel_event_id)) = (
        ev.content
            .get("m.relates_to")
            .and_then(|r| r.get("rel_type"))
            .and_then(|v| v.as_str()),
        ev.content
            .get("m.relates_to")
            .and_then(|r| r.get("event_id"))
            .and_then(|v| v.as_str()),
    ) {
        sqlx::query(
            r#"INSERT IGNORE INTO event_relations (event_id, room_id, rel_type, relates_to_event_id)
               VALUES (?, ?, ?, ?)"#,
        )
        .bind(ev.event_id)
        .bind(ev.room_id)
        .bind(rel_type)
        .bind(rel_event_id)
        .execute(pool)
        .await?;
    }

    Ok(())
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

    let Some(r) = row else {
        return Ok(None);
    };

    let content_str: String = r.get("content");
    let state_key: Option<String> = r.get("state_key");
    let fetched_event_id: String = r.get("event_id");
    let mut event = serde_json::json!({
        "event_id": &fetched_event_id,
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

    // unsigned.m.relations を付与（m.replace / m.reaction）
    let agg = crate::relations::get_aggregations_batch(pool, &[fetched_event_id]).await?;
    if let Some(relations) = agg.into_values().next() {
        event["unsigned"] = serde_json::json!({ "m.relations": relations });
    }

    Ok(Some(event))
}

/// イベントの stream_ordering を取得する。存在しない場合は None。
pub async fn get_stream_ordering(pool: &MySqlPool, event_id: &str) -> Result<Option<i64>> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT stream_ordering FROM events WHERE event_id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(o,)| o))
}

/// タイムスタンプに最も近いイベントを返す（MSC3030 / timestamp_to_event）。
///
/// - dir: "f" → ts 以降で最も古いイベント、"b" → ts 以前で最も新しいイベント
/// - 返り値: (event_id, origin_server_ts_ms)
pub async fn get_closest_event(
    pool: &MySqlPool,
    room_id: &str,
    ts_ms: i64,
    dir: &str,
) -> Result<Option<(String, i64)>> {
    use sqlx::Row;

    // created_at を ms に変換して比較する。
    // UNIX_TIMESTAMP(created_at) は秒精度なので * 1000 して ms にする。
    // created_at は DATETIME(3) なので小数部も含む。
    let (cmp_op, order_by) = if dir == "f" {
        (">=", "ASC")
    } else {
        ("<=", "DESC")
    };

    let sql = format!(
        r#"SELECT event_id,
                  CAST(UNIX_TIMESTAMP(created_at) * 1000 AS SIGNED) AS ts_ms
           FROM events
           WHERE room_id = ?
             AND UNIX_TIMESTAMP(created_at) * 1000 {cmp_op} ?
           ORDER BY created_at {order_by}
           LIMIT 1"#,
    );

    let row = sqlx::query(&sql)
        .bind(room_id)
        .bind(ts_ms)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| (r.get::<String, _>("event_id"), r.get::<i64, _>("ts_ms"))))
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

    let event_ids: Vec<String> = rows.iter().map(|r| r.get("event_id")).collect();
    let mut agg = crate::relations::get_aggregations_batch(pool, &event_ids)
        .await
        .unwrap_or_default();

    let events = rows
        .iter()
        .map(|r| {
            let event_id: String = r.get("event_id");
            let content_str: String = r.get("content");
            let mut ev = serde_json::json!({
                "event_id": &event_id,
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                    .and_utc()
                    .timestamp_millis(),
                "room_id": room_id,
            });
            if let Some(relations) = agg.remove(&event_id) {
                ev["unsigned"] = serde_json::json!({ "m.relations": relations });
            }
            ev
        })
        .collect();

    Ok(MessagePage { events, start, end })
}

/// イベントの content を `{}` に置き換える（redaction 用）。
/// 対象イベントが存在しない場合は何もしない（べき等）。
pub async fn redact_event(pool: &MySqlPool, event_id: &str) -> Result<()> {
    sqlx::query("UPDATE events SET content = '{}' WHERE event_id = ?")
        .bind(event_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `/context` レスポンス
pub struct EventContextResult {
    pub event: serde_json::Value,
    pub events_before: Vec<serde_json::Value>,
    pub events_after: Vec<serde_json::Value>,
    pub start: String,
    pub end: String,
}

/// 指定イベントの前後イベントを取得する（`/context` 用）。
/// イベントが存在しない場合は None を返す。
pub async fn get_context(
    pool: &MySqlPool,
    room_id: &str,
    event_id: &str,
    limit: u32,
) -> Result<Option<EventContextResult>> {
    use sqlx::Row;

    let half = (limit / 2).max(1) as i64;

    // 対象イベントの stream_ordering を取得
    let center_row =
        sqlx::query("SELECT stream_ordering FROM events WHERE room_id = ? AND event_id = ?")
            .bind(room_id)
            .bind(event_id)
            .fetch_optional(pool)
            .await?;

    let center_ord: u64 = match center_row {
        Some(r) => r.get("stream_ordering"),
        None => return Ok(None),
    };

    // 対象イベント本体を取得
    let event = get_by_id(pool, event_id).await?.unwrap_or_default();

    // 前のイベント（stream_ordering < center）を降順で取得
    let before_rows = sqlx::query(
        "SELECT event_id, sender, event_type, state_key, content, created_at, stream_ordering \
         FROM events WHERE room_id = ? AND stream_ordering < ? \
         ORDER BY stream_ordering DESC LIMIT ?",
    )
    .bind(room_id)
    .bind(center_ord)
    .bind(half)
    .fetch_all(pool)
    .await?;

    // 後のイベント（stream_ordering > center）を昇順で取得
    let after_rows = sqlx::query(
        "SELECT event_id, sender, event_type, state_key, content, created_at, stream_ordering \
         FROM events WHERE room_id = ? AND stream_ordering > ? \
         ORDER BY stream_ordering ASC LIMIT ?",
    )
    .bind(room_id)
    .bind(center_ord)
    .bind(half)
    .fetch_all(pool)
    .await?;

    let start = before_rows
        .last()
        .map(|r| ordering_to_token(r.get::<u64, _>("stream_ordering")))
        .unwrap_or_else(|| ordering_to_token(center_ord));
    let end = after_rows
        .last()
        .map(|r| ordering_to_token(r.get::<u64, _>("stream_ordering")))
        .unwrap_or_else(|| ordering_to_token(center_ord));

    // 前後イベントの集計（unsigned.m.relations）
    let context_ids: Vec<String> = before_rows
        .iter()
        .chain(after_rows.iter())
        .map(|r| r.get::<String, _>("event_id"))
        .collect();
    let mut ctx_agg = crate::relations::get_aggregations_batch(pool, &context_ids)
        .await
        .unwrap_or_default();

    let row_to_json_with_agg =
        |r: &sqlx::mysql::MySqlRow,
         agg: &mut std::collections::HashMap<String, serde_json::Value>| {
            let event_id: String = r.get("event_id");
            let content_str: String = r.get("content");
            let state_key: Option<String> = r.get("state_key");
            let mut ev = serde_json::json!({
                "event_id": &event_id,
                "room_id": room_id,
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                    .and_utc()
                    .timestamp_millis(),
            });
            if let Some(sk) = state_key {
                ev["state_key"] = serde_json::Value::String(sk);
            }
            if let Some(relations) = agg.remove(&event_id) {
                ev["unsigned"] = serde_json::json!({ "m.relations": relations });
            }
            ev
        };

    // events_before は時系列順（昇順）で返す
    let events_before: Vec<serde_json::Value> = before_rows
        .iter()
        .rev()
        .map(|r| row_to_json_with_agg(r, &mut ctx_agg))
        .collect();
    let events_after: Vec<serde_json::Value> = after_rows
        .iter()
        .map(|r| row_to_json_with_agg(r, &mut ctx_agg))
        .collect();

    Ok(Some(EventContextResult {
        event,
        events_before,
        events_after,
        start,
        end,
    }))
}

/// ルームイベントの全文検索（LIKE ベース）。
/// ユーザーが参加しているルームのうち、search_term を body フィールドに含む
/// m.room.message イベントを返す。
pub async fn search_room_events(
    pool: &MySqlPool,
    user_id: &str,
    search_term: &str,
    rooms: Option<&[String]>,
    order_by: Option<&str>,
    before_ordering: Option<i64>,
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    let _order_by = order_by; // 現時点では stream_ordering DESC 固定
    let like_pattern = format!("%{}%", search_term.replace('%', "\\%").replace('_', "\\_"));

    // カーソル条件: before_ordering が Some の場合は stream_ordering < before_ordering
    let cursor_clause = if before_ordering.is_some() {
        "AND e.stream_ordering < ?"
    } else {
        ""
    };

    let rows = if let Some(room_list) = rooms {
        if room_list.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = room_list.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            r#"SELECT e.event_id, e.room_id, e.sender, e.event_type, e.content,
                      e.stream_ordering, e.created_at
               FROM events e
               INNER JOIN room_memberships rm ON rm.room_id = e.room_id
                 AND rm.user_id = ? AND rm.membership = 'join'
               WHERE e.event_type = 'm.room.message'
                 AND e.state_key IS NULL
                 AND e.room_id IN ({placeholders})
                 AND JSON_UNQUOTE(JSON_EXTRACT(e.content, '$.body')) LIKE ?
                 {cursor_clause}
               ORDER BY e.stream_ordering DESC
               LIMIT ?"#,
        );
        let mut q = sqlx::query(&sql).bind(user_id);
        for r in room_list {
            q = q.bind(r);
        }
        q = q.bind(&like_pattern);
        if let Some(ord) = before_ordering {
            q = q.bind(ord);
        }
        q.bind(limit).fetch_all(pool).await?
    } else {
        let sql = format!(
            r#"SELECT e.event_id, e.room_id, e.sender, e.event_type, e.content,
                      e.stream_ordering, e.created_at
               FROM events e
               INNER JOIN room_memberships rm ON rm.room_id = e.room_id
                 AND rm.user_id = ? AND rm.membership = 'join'
               WHERE e.event_type = 'm.room.message'
                 AND e.state_key IS NULL
                 AND JSON_UNQUOTE(JSON_EXTRACT(e.content, '$.body')) LIKE ?
                 {cursor_clause}
               ORDER BY e.stream_ordering DESC
               LIMIT ?"#,
        );
        let mut q = sqlx::query(&sql).bind(user_id).bind(&like_pattern);
        if let Some(ord) = before_ordering {
            q = q.bind(ord);
        }
        q.bind(limit).fetch_all(pool).await?
    };

    Ok(rows
        .into_iter()
        .map(|r| {
            let content_str: String = r.get("content");
            serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "room_id": r.get::<String, _>("room_id"),
                "sender": r.get::<String, _>("sender"),
                "type": r.get::<String, _>("event_type"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": r.get::<chrono::NaiveDateTime, _>("created_at")
                    .and_utc()
                    .timestamp_millis(),
                "stream_ordering": r.get::<i64, _>("stream_ordering"),
            })
        })
        .collect())
}
