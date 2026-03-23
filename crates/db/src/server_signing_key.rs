/// サーバー署名鍵の永続化
///
/// 起動時に DB から鍵を読み込み、存在しない場合は生成して保存する。
/// 非マクロ sqlx を使用（.sqlx/ メタデータ未生成のため）。
use anyhow::Result;
use sqlx::MySqlPool;

/// 指定 key_id の秘密鍵（unpadded base64）を返す。存在しない場合は None。
pub async fn load(pool: &MySqlPool, key_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT private_key FROM server_signing_key WHERE key_id = ?")
            .bind(key_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(k,)| k))
}

/// 秘密鍵を保存する（既存の場合は上書き）。
pub async fn save(pool: &MySqlPool, key_id: &str, private_key_b64: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO server_signing_key (key_id, private_key) VALUES (?, ?)
         ON DUPLICATE KEY UPDATE private_key = VALUES(private_key)",
    )
    .bind(key_id)
    .bind(private_key_b64)
    .execute(pool)
    .await?;
    Ok(())
}
