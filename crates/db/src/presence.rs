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

/// ルームのメンバー全員のプレゼンスを取得（sync 用）
pub async fn get_for_room_members(pool: &MySqlPool, room_id: &str) -> Result<Vec<PresenceStatus>> {
    let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        r#"SELECT p.user_id, p.presence, p.status_msg, p.last_active_ts
           FROM presence p
           INNER JOIN room_memberships rm ON rm.user_id = p.user_id
           WHERE rm.room_id = ? AND rm.membership = 'join'"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

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
