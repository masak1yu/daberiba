use anyhow::Result;
use sqlx::MySqlPool;

/// ユーザーに紐づく 3pid 一覧を返す。
pub async fn list(pool: &MySqlPool, user_id: &str) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    let rows = sqlx::query(
        "SELECT medium, address, validated_at, added_at FROM user_threepids WHERE user_id = ? ORDER BY added_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let medium: String = r.get("medium");
            let address: String = r.get("address");
            let validated_at: i64 = r.get("validated_at");
            let added_at: i64 = r.get("added_at");
            serde_json::json!({
                "medium": medium,
                "address": address,
                "validated_at": validated_at,
                "added_at": added_at,
            })
        })
        .collect())
}

/// 3pid を追加する。同一 medium + address が既に存在する場合は更新。
pub async fn add(pool: &MySqlPool, user_id: &str, medium: &str, address: &str) -> Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"INSERT INTO user_threepids (user_id, medium, address, validated_at, added_at)
           VALUES (?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE user_id = VALUES(user_id), validated_at = VALUES(validated_at)"#,
    )
    .bind(user_id)
    .bind(medium)
    .bind(address)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await?;

    Ok(())
}

/// 3pid を削除する。
pub async fn delete(pool: &MySqlPool, user_id: &str, medium: &str, address: &str) -> Result<()> {
    sqlx::query("DELETE FROM user_threepids WHERE user_id = ? AND medium = ? AND address = ?")
        .bind(user_id)
        .bind(medium)
        .bind(address)
        .execute(pool)
        .await?;

    Ok(())
}
