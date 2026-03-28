use anyhow::Result;
use sqlx::MySqlPool;

pub struct PresenceStatus {
    pub user_id: String,
    pub presence: String,
    pub status_msg: Option<String>,
    pub last_active_ts: i64,
}

/// プレゼンス状態を upsert
pub async fn set(
    pool: &MySqlPool,
    user_id: &str,
    presence: &str,
    status_msg: Option<&str>,
) -> Result<()> {
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"INSERT INTO presence (user_id, presence, status_msg, last_active_ts)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE presence = VALUES(presence),
                                   status_msg = VALUES(status_msg),
                                   last_active_ts = VALUES(last_active_ts)"#,
    )
    .bind(user_id)
    .bind(presence)
    .bind(status_msg)
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(())
}

/// 単一ユーザーのプレゼンス取得
pub async fn get(pool: &MySqlPool, user_id: &str) -> Result<Option<PresenceStatus>> {
    let row: Option<(String, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT user_id, presence, status_msg, last_active_ts FROM presence WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(user_id, presence, status_msg, last_active_ts)| PresenceStatus {
            user_id,
            presence,
            status_msg,
            last_active_ts,
        },
    ))
}

/// since_ms 以降に更新されたプレゼンスのみ取得（sync デルタ用）。
/// user_ids が空の場合は空を返す。
pub async fn get_changed_since(
    pool: &MySqlPool,
    user_ids: &[String],
    since_ms: i64,
) -> Result<Vec<PresenceStatus>> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT user_id, presence, status_msg, last_active_ts
         FROM presence
         WHERE user_id IN ({placeholders}) AND last_active_ts > ?"
    );
    let mut q = sqlx::query_as::<_, (String, String, Option<String>, i64)>(&sql);
    for uid in user_ids {
        q = q.bind(uid);
    }
    q = q.bind(since_ms);
    let rows = q.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(user_id, presence, status_msg, last_active_ts)| PresenceStatus {
                user_id,
                presence,
                status_msg,
                last_active_ts,
            },
        )
        .collect())
}

/// ルームのメンバー全員のプレゼンスを取得（sync 用）。
/// presence レコードがないユーザーは "offline" をデフォルトとして返す。
pub async fn get_for_room_members(pool: &MySqlPool, room_id: &str) -> Result<Vec<PresenceStatus>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT rm.user_id,
                  COALESCE(p.presence, 'offline') AS presence,
                  p.status_msg,
                  COALESCE(p.last_active_ts, 0) AS last_active_ts
           FROM room_memberships rm
           LEFT JOIN presence p ON p.user_id = rm.user_id
           WHERE rm.room_id = ? AND rm.membership = 'join'"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| PresenceStatus {
            user_id: r.get("user_id"),
            presence: r.get("presence"),
            status_msg: r.get("status_msg"),
            last_active_ts: r.get("last_active_ts"),
        })
        .collect())
}

/// sync 時に last_active_ts を現在時刻に更新する。
/// presence レコードがない場合は 'online' で新規作成する。
pub async fn set_active(pool: &MySqlPool, user_id: &str) -> Result<()> {
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"INSERT INTO presence (user_id, presence, status_msg, last_active_ts)
           VALUES (?, 'online', NULL, ?)
           ON DUPLICATE KEY UPDATE last_active_ts = VALUES(last_active_ts)"#,
    )
    .bind(user_id)
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(())
}
