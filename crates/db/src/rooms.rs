use anyhow::Result;
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

pub async fn create(
    pool: &MySqlPool,
    creator_user_id: &str,
    name: Option<&str>,
    topic: Option<&str>,
    server_name: &str,
) -> Result<String> {
    let room_id = format!("!{}:{}", Uuid::new_v4().to_string().replace('-', ""), server_name);

    sqlx::query("INSERT INTO rooms (room_id, creator_user_id, name, topic) VALUES (?, ?, ?, ?)")
        .bind(&room_id)
        .bind(creator_user_id)
        .bind(name)
        .bind(topic)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO room_memberships (room_id, user_id, membership) VALUES (?, ?, 'join')",
    )
    .bind(&room_id)
    .bind(creator_user_id)
    .execute(pool)
    .await?;

    Ok(room_id)
}

pub async fn join(pool: &MySqlPool, user_id: &str, room_id: &str) -> Result<String> {
    sqlx::query(
        r#"INSERT INTO room_memberships (room_id, user_id, membership)
           VALUES (?, ?, 'join')
           ON DUPLICATE KEY UPDATE membership = 'join'"#,
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(room_id.to_string())
}

pub async fn leave(pool: &MySqlPool, user_id: &str, room_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE room_memberships SET membership = 'leave' WHERE room_id = ? AND user_id = ?",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn joined_rooms(pool: &MySqlPool, user_id: &str) -> Result<Vec<String>> {
    let rows =
        sqlx::query("SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'")
            .bind(user_id)
            .fetch_all(pool)
            .await?;

    Ok(rows.into_iter().map(|r| r.get::<String, _>("room_id")).collect())
}
