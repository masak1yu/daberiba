use anyhow::Result;
use sqlx::MySqlPool;

/// エイリアスを作成（既存エイリアスがあれば 409 用に Err）
pub async fn create(pool: &MySqlPool, alias: &str, room_id: &str, creator: &str) -> Result<()> {
    sqlx::query("INSERT INTO room_aliases (alias, room_id, creator) VALUES (?, ?, ?)")
        .bind(alias)
        .bind(room_id)
        .bind(creator)
        .execute(pool)
        .await?;
    Ok(())
}

/// エイリアスからルーム ID を解決
pub async fn resolve(pool: &MySqlPool, alias: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT room_id FROM room_aliases WHERE alias = ?")
        .bind(alias)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(room_id,)| room_id))
}

/// エイリアスを削除（呼び出し元で権限チェック済み前提）
pub async fn delete(pool: &MySqlPool, alias: &str) -> Result<bool> {
    let res = sqlx::query("DELETE FROM room_aliases WHERE alias = ?")
        .bind(alias)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// ルームに紐づく全エイリアスを返す
pub async fn list_for_room(pool: &MySqlPool, room_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT alias FROM room_aliases WHERE room_id = ? ORDER BY alias")
            .bind(room_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(a,)| a).collect())
}

/// エイリアスの creator を返す（権限チェック用）
pub async fn get_creator(pool: &MySqlPool, alias: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT creator FROM room_aliases WHERE alias = ?")
        .bind(alias)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(creator,)| creator))
}
