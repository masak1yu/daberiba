use anyhow::Result;
use sqlx::MySqlPool;

/// フィルターを保存して filter_id を返す（POST /user/{userId}/filter）
pub async fn create(pool: &MySqlPool, user_id: &str, filter_json: &str) -> Result<u64> {
    let result = sqlx::query("INSERT INTO filters (user_id, filter) VALUES (?, ?)")
        .bind(user_id)
        .bind(filter_json)
        .execute(pool)
        .await?;
    Ok(result.last_insert_id())
}

/// フィルターを取得（GET /user/{userId}/filter/{filterId}）
pub async fn get(pool: &MySqlPool, user_id: &str, filter_id: u64) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT filter FROM filters WHERE filter_id = ? AND user_id = ?")
            .bind(filter_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(f,)| f))
}
