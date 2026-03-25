use anyhow::Result;
use sqlx::MySqlPool;

pub async fn get_all(pool: &MySqlPool, room_id: &str) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query!(
        r#"SELECT e.event_id, e.sender, e.event_type, e.state_key, e.content, e.created_at
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ?"#,
        room_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let content_str = r.content;
            serde_json::json!({
                "event_id": r.event_id,
                "sender": r.sender,
                "type": r.event_type,
                "state_key": r.state_key,
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": r.created_at.and_utc().timestamp_millis(),
                "room_id": room_id,
            })
        })
        .collect())
}

/// send_join レスポンス用の auth_chain イベントを返す。
///
/// auth_chain は「現在のステートを認可するイベント群」。
/// ここでは m.room.create / m.room.join_rules / m.room.power_levels を返す
/// （これらが存在すれば join 認可に必要な最小 auth chain となる）。
pub async fn get_auth_events(pool: &MySqlPool, room_id: &str) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"SELECT e.event_id, e.sender, e.event_type, rs.state_key, e.content, e.created_at
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ?
             AND rs.event_type IN ('m.room.create', 'm.room.join_rules', 'm.room.power_levels')"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
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
        .collect())
}

/// make_join テンプレート用: auth_events として必要なイベントの ID を返す。
/// m.room.create / m.room.join_rules / m.room.power_levels の event_id リスト。
pub async fn get_auth_event_ids(pool: &MySqlPool, room_id: &str) -> Result<Vec<String>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT rs.event_id
           FROM room_state rs
           WHERE rs.room_id = ?
             AND rs.event_type IN ('m.room.create', 'm.room.join_rules', 'm.room.power_levels')"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("event_id"))
        .collect())
}

/// ルーム内のユーザーのパワーレベルを返す。
/// power_levels イベントが存在しない場合はデフォルト値（0）を返す。
pub async fn get_user_power_level(pool: &MySqlPool, room_id: &str, user_id: &str) -> Result<i64> {
    let pl = get_event(pool, room_id, "m.room.power_levels", "").await?;
    Ok(pl
        .map(|v| {
            // users マップから個別設定を優先し、なければ users_default
            let user_pl = v
                .get("users")
                .and_then(|u| u.get(user_id))
                .and_then(|p| p.as_i64());
            let default_pl = v.get("users_default").and_then(|p| p.as_i64()).unwrap_or(0);
            user_pl.unwrap_or(default_pl)
        })
        .unwrap_or(0))
}

/// ルームのアクションに必要なパワーレベルを返す（kick, ban, redact, invite 等）。
/// power_levels イベントが存在しない場合はデフォルト値（50）を返す。
pub async fn get_required_power_level(
    pool: &MySqlPool,
    room_id: &str,
    action: &str,
) -> Result<i64> {
    let pl = get_event(pool, room_id, "m.room.power_levels", "").await?;
    Ok(pl
        .map(|v| v.get(action).and_then(|p| p.as_i64()).unwrap_or(50))
        .unwrap_or(50))
}

/// 指定 stream_ordering 時点のルームステートスナップショットを返す。
/// 各 (event_type, state_key) ペアについて stream_ordering <= at_ordering の
/// 最新イベントを取得する（相関サブクエリ使用）。
pub async fn get_state_at(
    pool: &MySqlPool,
    room_id: &str,
    at_ordering: u64,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    let rows = sqlx::query(
        r#"SELECT e.event_id, e.sender, e.event_type, e.state_key, e.content, e.created_at
           FROM events e
           WHERE e.room_id = ?
             AND e.state_key IS NOT NULL
             AND e.stream_ordering <= ?
             AND e.stream_ordering = (
                 SELECT MAX(e2.stream_ordering)
                 FROM events e2
                 WHERE e2.room_id = e.room_id
                   AND e2.event_type = e.event_type
                   AND e2.state_key = e.state_key
                   AND e2.stream_ordering <= ?
             )
           ORDER BY e.event_type ASC, e.state_key ASC"#,
    )
    .bind(room_id)
    .bind(at_ordering)
    .bind(at_ordering)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
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
        .collect())
}

pub async fn get_event(
    pool: &MySqlPool,
    room_id: &str,
    event_type: &str,
    state_key: &str,
) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query!(
        r#"SELECT e.content
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ? AND rs.event_type = ? AND rs.state_key = ?"#,
        room_id,
        event_type,
        state_key
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| serde_json::from_str::<serde_json::Value>(&r.content).unwrap_or_default()))
}
