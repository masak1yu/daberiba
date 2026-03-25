use anyhow::Result;
use sqlx::MySqlPool;
use std::collections::HashMap;

/// m.relates_to を持つイベントのリレーション情報を記録する。
/// 既に同一 event_id が存在する場合は無視する（INSERT IGNORE）。
pub async fn record(
    pool: &MySqlPool,
    event_id: &str,
    room_id: &str,
    rel_type: &str,
    relates_to_event_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO event_relations (event_id, room_id, rel_type, relates_to_event_id)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(event_id)
    .bind(room_id)
    .bind(rel_type)
    .bind(relates_to_event_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// イベント ID のリストに対して `unsigned.m.relations` の集計データを一括取得する。
///
/// 返り値: event_id -> { "m.replace": {...}, "m.reaction": {"chunk": [...]} }
/// - m.replace: stream_ordering 最大の置換イベント（event_id / sender / origin_server_ts）
/// - m.reaction: key ごとの件数リスト
pub async fn get_aggregations_batch(
    pool: &MySqlPool,
    event_ids: &[String],
) -> Result<HashMap<String, serde_json::Value>> {
    use sqlx::Row;

    if event_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = event_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

    // --- m.replace: 最新の置換イベントを取得 ---
    let replace_sql = format!(
        r#"SELECT er.relates_to_event_id,
                  e.event_id, e.sender,
                  e.created_at
           FROM event_relations er
           INNER JOIN events e ON e.event_id = er.event_id
           WHERE er.relates_to_event_id IN ({placeholders})
             AND er.rel_type = 'm.replace'
           ORDER BY e.stream_ordering DESC"#,
    );
    let mut q = sqlx::query(&replace_sql);
    for id in event_ids {
        q = q.bind(id);
    }
    let replace_rows = q.fetch_all(pool).await?;

    // relates_to_event_id ごとに最初の行（= stream_ordering 最大）を採用
    let mut replace_map: HashMap<String, serde_json::Value> = HashMap::new();
    for row in &replace_rows {
        let target: String = row.get("relates_to_event_id");
        replace_map.entry(target).or_insert_with(|| {
            serde_json::json!({
                "event_id": row.get::<String, _>("event_id"),
                "sender": row.get::<String, _>("sender"),
                "origin_server_ts": row.get::<chrono::NaiveDateTime, _>("created_at")
                    .and_utc()
                    .timestamp_millis(),
            })
        });
    }

    // --- m.reaction: key ごとの件数 ---
    let reaction_sql = format!(
        r#"SELECT er.relates_to_event_id,
                  JSON_UNQUOTE(JSON_EXTRACT(e.content, '$."m.relates_to".key')) AS reaction_key,
                  COUNT(*) AS cnt
           FROM event_relations er
           INNER JOIN events e ON e.event_id = er.event_id
           WHERE er.relates_to_event_id IN ({placeholders})
             AND er.rel_type = 'm.reaction'
           GROUP BY er.relates_to_event_id, reaction_key"#,
    );
    let mut q = sqlx::query(&reaction_sql);
    for id in event_ids {
        q = q.bind(id);
    }
    let reaction_rows = q.fetch_all(pool).await?;

    // relates_to_event_id ごとに chunk を組み立て
    let mut reaction_map: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for row in &reaction_rows {
        let target: String = row.get("relates_to_event_id");
        let key: Option<String> = row.try_get("reaction_key").ok().flatten();
        let cnt: i64 = row.get("cnt");
        if let Some(k) = key {
            reaction_map
                .entry(target)
                .or_default()
                .push(serde_json::json!({
                    "type": "m.reaction",
                    "key": k,
                    "count": cnt,
                }));
        }
    }

    // --- 集計結果を event_id ごとにまとめる ---
    let mut result: HashMap<String, serde_json::Value> = HashMap::new();
    for id in event_ids {
        let mut relations = serde_json::Map::new();
        if let Some(rep) = replace_map.get(id) {
            relations.insert("m.replace".to_string(), rep.clone());
        }
        if let Some(chunk) = reaction_map.get(id) {
            relations.insert(
                "m.reaction".to_string(),
                serde_json::json!({ "chunk": chunk }),
            );
        }
        if !relations.is_empty() {
            result.insert(id.clone(), serde_json::Value::Object(relations));
        }
    }

    Ok(result)
}

pub struct RelationPage {
    pub events: Vec<serde_json::Value>,
    /// 次ページの先頭となる event_id（これ以降を取得する場合の from トークン）
    pub next_batch: Option<String>,
    /// 前ページの先頭となる event_id
    pub prev_batch: Option<String>,
}

/// 指定イベントへのリレーションを取得する（ページネーション付き）。
/// - rel_type: None の場合は全リレーション
/// - event_type: None の場合は全イベント種別
/// - from: ページネーショントークン（event_id の文字列表現）
/// - limit: 取得件数（最大 50）
pub async fn list(
    pool: &MySqlPool,
    room_id: &str,
    relates_to_event_id: &str,
    rel_type: Option<&str>,
    event_type: Option<&str>,
    from: Option<&str>,
    limit: u32,
) -> Result<RelationPage> {
    use sqlx::Row;

    // カーソル: from が指定された場合は from より新しいイベントのみ
    // stream_ordering を用いたカーソルのために from を event_id として使う

    let mut conditions = vec![
        "er.relates_to_event_id = ?".to_string(),
        "er.room_id = ?".to_string(),
    ];
    if rel_type.is_some() {
        conditions.push("er.rel_type = ?".to_string());
    }
    if event_type.is_some() {
        conditions.push("e.event_type = ?".to_string());
    }
    // from カーソル: from の event_id の stream_ordering より大きいもの
    if from.is_some() {
        conditions.push(
            "e.stream_ordering > (SELECT stream_ordering FROM events WHERE event_id = ?)"
                .to_string(),
        );
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        r#"SELECT e.event_id, e.room_id, e.sender, e.event_type,
                  e.content, e.stream_ordering, e.created_at,
                  er.rel_type
           FROM event_relations er
           INNER JOIN events e ON e.event_id = er.event_id
           WHERE {where_clause}
           ORDER BY e.stream_ordering ASC
           LIMIT ?"#,
    );

    let mut q = sqlx::query(&sql).bind(relates_to_event_id).bind(room_id);
    if let Some(rt) = rel_type {
        q = q.bind(rt);
    }
    if let Some(et) = event_type {
        q = q.bind(et);
    }
    if let Some(f) = from {
        q = q.bind(f);
    }
    // limit+1 件取得してページ継続を判定
    let rows = q.bind(limit + 1).fetch_all(pool).await?;

    let has_more = rows.len() as u32 == limit + 1;
    let rows = if has_more {
        &rows[..rows.len() - 1]
    } else {
        &rows[..]
    };

    let events: Vec<serde_json::Value> = rows
        .iter()
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
            })
        })
        .collect();

    let next_batch = if has_more {
        rows.last().map(|r| r.get::<String, _>("event_id"))
    } else {
        None
    };

    let prev_batch = from.map(|f| f.to_string());

    Ok(RelationPage {
        events,
        next_batch,
        prev_batch,
    })
}
