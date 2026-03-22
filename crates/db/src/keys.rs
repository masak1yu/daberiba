use anyhow::Result;
use sqlx::MySqlPool;

/// デバイス公開鍵をアップロード / 更新（PUT /keys/upload）
pub async fn upload_device_keys(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    key_json: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO device_keys (user_id, device_id, key_json)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE key_json = VALUES(key_json)"#,
    )
    .bind(user_id)
    .bind(device_id)
    .bind(key_json)
    .execute(pool)
    .await?;
    Ok(())
}

/// ワンタイム鍵をアップロード（既存の key_id は無視）
pub async fn upload_one_time_keys(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    keys: &[(String, String)], // (key_id, key_json)
) -> Result<()> {
    for (key_id, key_json) in keys {
        sqlx::query(
            r#"INSERT IGNORE INTO one_time_keys (user_id, device_id, key_id, key_json)
               VALUES (?, ?, ?, ?)"#,
        )
        .bind(user_id)
        .bind(device_id)
        .bind(key_id)
        .bind(key_json)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// ユーザーのデバイス鍵を取得（POST /keys/query）
/// device_ids が空なら全デバイス
pub async fn get_device_keys(
    pool: &MySqlPool,
    user_id: &str,
    device_ids: &[String],
) -> Result<Vec<(String, String)>> {
    // (device_id, key_json)
    if device_ids.is_empty() {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT device_id, key_json FROM device_keys WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
        return Ok(rows);
    }
    let placeholders = device_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT device_id, key_json FROM device_keys WHERE user_id = ? AND device_id IN ({placeholders})"
    );
    let mut q = sqlx::query_as(&sql).bind(user_id);
    for id in device_ids {
        q = q.bind(id);
    }
    let rows: Vec<(String, String)> = q.fetch_all(pool).await?;
    Ok(rows)
}

/// ワンタイム鍵を 1 件 claim して削除（POST /keys/claim）
pub async fn claim_one_time_key(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
    key_algorithm: &str,
) -> Result<Option<(String, String)>> {
    // algorithm プレフィックスで絞り込み
    let prefix = format!("{key_algorithm}:%");
    let row: Option<(u64, String, String)> = sqlx::query_as(
        r#"SELECT id, key_id, key_json FROM one_time_keys
           WHERE user_id = ? AND device_id = ? AND key_id LIKE ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(device_id)
    .bind(&prefix)
    .fetch_optional(pool)
    .await?;

    if let Some((id, key_id, key_json)) = row {
        sqlx::query("DELETE FROM one_time_keys WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(Some((key_id, key_json)))
    } else {
        Ok(None)
    }
}

/// デバイスごとのワンタイム鍵残数（POST /keys/upload のレスポンス用）
pub async fn count_one_time_keys(
    pool: &MySqlPool,
    user_id: &str,
    device_id: &str,
) -> Result<std::collections::HashMap<String, i64>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT SUBSTRING_INDEX(key_id, ':', 1) AS algo, COUNT(*) AS cnt
           FROM one_time_keys WHERE user_id = ? AND device_id = ?
           GROUP BY algo"#,
    )
    .bind(user_id)
    .bind(device_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}
