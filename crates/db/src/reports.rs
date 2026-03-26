use anyhow::Result;
use sqlx::MySqlPool;

/// コンテンツ報告を記録する（POST /rooms/{roomId}/report/{eventId}）。
pub async fn create(
    pool: &MySqlPool,
    room_id: &str,
    event_id: &str,
    user_id: &str,
    score: Option<i32>,
    reason: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO event_reports (room_id, event_id, user_id, score, reason) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(room_id)
    .bind(event_id)
    .bind(user_id)
    .bind(score)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(())
}

/// 管理者向け: 全レポート一覧を返す。
pub async fn list_all(pool: &MySqlPool) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT id, room_id, event_id, user_id, score, reason, created_at \
         FROM event_reports ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let created_at: chrono::NaiveDateTime = r.get("created_at");
            serde_json::json!({
                "id": r.get::<u64, _>("id"),
                "room_id": r.get::<String, _>("room_id"),
                "event_id": r.get::<String, _>("event_id"),
                "user_id": r.get::<String, _>("user_id"),
                "score": r.get::<Option<i32>, _>("score"),
                "reason": r.get::<Option<String>, _>("reason"),
                "received_ts": created_at.and_utc().timestamp_millis(),
            })
        })
        .collect())
}
