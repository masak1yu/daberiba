use anyhow::Result;
use sqlx::MySqlPool;
use uuid::Uuid;

pub async fn create(
    pool: &MySqlPool,
    creator_user_id: &str,
    name: Option<&str>,
    topic: Option<&str>,
    server_name: &str,
) -> Result<String> {
    let room_id = format!(
        "!{}:{}",
        Uuid::new_v4().to_string().replace('-', ""),
        server_name
    );

    sqlx::query!(
        "INSERT INTO rooms (room_id, creator_user_id, name, topic) VALUES (?, ?, ?, ?)",
        room_id,
        creator_user_id,
        name,
        topic
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        "INSERT INTO room_memberships (room_id, user_id, membership) VALUES (?, ?, 'join')",
        room_id,
        creator_user_id
    )
    .execute(pool)
    .await?;

    Ok(room_id)
}

pub async fn join(pool: &MySqlPool, user_id: &str, room_id: &str) -> Result<String> {
    sqlx::query!(
        r#"INSERT INTO room_memberships (room_id, user_id, membership)
           VALUES (?, ?, 'join')
           ON DUPLICATE KEY UPDATE membership = 'join'"#,
        room_id,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(room_id.to_string())
}

pub async fn leave(pool: &MySqlPool, user_id: &str, room_id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE room_memberships SET membership = 'leave' WHERE room_id = ? AND user_id = ?",
        room_id,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn joined_rooms(pool: &MySqlPool, user_id: &str) -> Result<Vec<String>> {
    let rows = sqlx::query!(
        "SELECT room_id FROM room_memberships WHERE user_id = ? AND membership = 'join'",
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.room_id).collect())
}

pub async fn get_members(pool: &MySqlPool, room_id: &str) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query!(
        r#"SELECT rm.user_id, rm.membership, u.display_name, u.avatar_url
           FROM room_memberships rm
           JOIN users u ON u.user_id = rm.user_id
           WHERE rm.room_id = ?"#,
        room_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "type": "m.room.member",
                "state_key": r.user_id,
                "content": {
                    "membership": r.membership,
                    "displayname": r.display_name,
                    "avatar_url": r.avatar_url,
                },
            })
        })
        .collect())
}

pub async fn get_joined_members(
    pool: &MySqlPool,
    room_id: &str,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let rows = sqlx::query!(
        r#"SELECT rm.user_id, u.display_name, u.avatar_url
           FROM room_memberships rm
           JOIN users u ON u.user_id = rm.user_id
           WHERE rm.room_id = ? AND rm.membership = 'join'"#,
        room_id
    )
    .fetch_all(pool)
    .await?;

    let mut map = serde_json::Map::new();
    for r in rows {
        map.insert(
            r.user_id,
            serde_json::json!({
                "display_name": r.display_name,
                "avatar_url": r.avatar_url,
            }),
        );
    }
    Ok(map)
}

pub async fn invite(
    pool: &MySqlPool,
    room_id: &str,
    _inviter_user_id: &str,
    invitee_user_id: &str,
) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO room_memberships (room_id, user_id, membership)
           VALUES (?, ?, 'invite')
           ON DUPLICATE KEY UPDATE membership = 'invite'"#,
        room_id,
        invitee_user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
