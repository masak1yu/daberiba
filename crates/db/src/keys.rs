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

/// /sync の device_lists.changed 用:
/// `since_ms` 以降に device_keys が更新されたユーザーのうち、
/// `user_id` と共有ルームにいるユーザーの user_id 一覧を返す。
/// `since_ms` が None（初回 sync）の場合は共有ルームの全ユーザーを返す。
pub async fn get_changed_users(
    pool: &MySqlPool,
    user_id: &str,
    since_ms: Option<u64>,
) -> Result<Vec<String>> {
    use sqlx::Row;
    let rows = if let Some(ms) = since_ms {
        // UNIX ミリ秒 → MySQL DATETIME 変換（FROM_UNIXTIME はマイクロ秒非対応のためミリ秒を秒に変換）
        let since_secs = (ms / 1000) as i64;
        sqlx::query(
            r#"SELECT DISTINCT dk.user_id
               FROM device_keys dk
               JOIN room_memberships rm ON rm.user_id = dk.user_id AND rm.membership = 'join'
               WHERE rm.room_id IN (
                   SELECT room_id FROM room_memberships
                   WHERE user_id = ? AND membership = 'join'
               )
               AND dk.user_id != ?
               AND UNIX_TIMESTAMP(dk.updated_at) >= ?"#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(since_secs)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"SELECT DISTINCT dk.user_id
               FROM device_keys dk
               JOIN room_memberships rm ON rm.user_id = dk.user_id AND rm.membership = 'join'
               WHERE rm.room_id IN (
                   SELECT room_id FROM room_memberships
                   WHERE user_id = ? AND membership = 'join'
               )
               AND dk.user_id != ?"#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?
    };
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("user_id"))
        .collect())
}

/// /sync の device_lists.left 用:
/// `since_stream` 以降に退出イベント（leave / ban）があり、
/// かつ現在 `user_id` と共有ルームがないユーザーの user_id 一覧を返す。
pub async fn get_left_users(
    pool: &MySqlPool,
    user_id: &str,
    since_stream: u64,
) -> Result<Vec<String>> {
    use sqlx::Row;
    // since_stream 以降に leave/ban になったユーザー（共有ルームで）
    let rows = sqlx::query(
        r#"SELECT DISTINCT e.sender AS left_user
           FROM events e
           WHERE e.room_id IN (
               SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'
           )
           AND e.event_type = 'm.room.member'
           AND JSON_UNQUOTE(JSON_EXTRACT(e.content, '$.membership')) IN ('leave', 'ban')
           AND e.stream_ordering > ?
           AND e.state_key != ?
           AND e.state_key NOT IN (
               SELECT rm2.user_id FROM room_memberships rm2
               WHERE rm2.user_id = e.state_key
                 AND rm2.membership = 'join'
                 AND rm2.room_id IN (
                     SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'
                 )
           )"#,
    )
    .bind(user_id)
    .bind(since_stream)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("left_user"))
        .collect())
}

/// クロスサイニングキーをアップロード / 更新
pub async fn upload_cross_signing_keys(
    pool: &MySqlPool,
    user_id: &str,
    key_type: &str,
    key_json: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO cross_signing_keys (user_id, key_type, key_json)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE key_json = VALUES(key_json)"#,
    )
    .bind(user_id)
    .bind(key_type)
    .bind(key_json)
    .execute(pool)
    .await?;
    Ok(())
}

/// ユーザーのクロスサイニングキーを全種類取得
pub async fn get_cross_signing_keys(
    pool: &MySqlPool,
    user_id: &str,
) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key_type, key_json FROM cross_signing_keys WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

/// キー署名をアップロード / 更新
pub async fn upload_key_signature(
    pool: &MySqlPool,
    signer_user_id: &str,
    target_user_id: &str,
    key_id: &str,
    signature_json: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO key_signatures (signer_user_id, target_user_id, key_id, signature_json)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE signature_json = VALUES(signature_json)"#,
    )
    .bind(signer_user_id)
    .bind(target_user_id)
    .bind(key_id)
    .bind(signature_json)
    .execute(pool)
    .await?;
    Ok(())
}

/// 対象ユーザーのキーに対する署名一覧を取得
pub async fn get_key_signatures(
    pool: &MySqlPool,
    target_user_id: &str,
    key_id: &str,
) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT signer_user_id, signature_json FROM key_signatures WHERE target_user_id = ? AND key_id = ?",
    )
    .bind(target_user_id)
    .bind(key_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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
