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

/// ルームのバージョンを返す。ルームが存在しない場合は None。
pub async fn get_version(pool: &MySqlPool, room_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT room_version FROM rooms WHERE room_id = ?")
        .bind(room_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(v,)| v))
}

/// ルームの room_version を更新する。
pub async fn set_version(pool: &MySqlPool, room_id: &str, version: &str) -> Result<()> {
    sqlx::query("UPDATE rooms SET room_version = ? WHERE room_id = ?")
        .bind(version)
        .bind(room_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// ルームの参加メンバー数を返す
pub async fn count_joined_members(pool: &MySqlPool, room_id: &str) -> Result<u64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM room_memberships WHERE room_id = ? AND membership = 'join'",
    )
    .bind(room_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0 as u64)
}

pub struct PublicRoom {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub num_joined_members: i64,
}

/// join_rules = public なルームを一覧取得
pub async fn get_public_rooms(pool: &MySqlPool) -> Result<Vec<PublicRoom>> {
    // room_state に m.room.join_rules が "public" なルームを探す
    let rows: Vec<(String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        r#"SELECT r.room_id, r.name, r.topic,
                  COUNT(rm.user_id) AS num_joined_members
           FROM rooms r
           JOIN room_state rs ON rs.room_id = r.room_id
                              AND rs.event_type = 'm.room.join_rules'
                              AND rs.state_key = ''
           JOIN events e ON e.event_id = rs.event_id
           JOIN room_memberships rm ON rm.room_id = r.room_id AND rm.membership = 'join'
           WHERE JSON_UNQUOTE(JSON_EXTRACT(e.content, '$.join_rule')) = 'public'
           GROUP BY r.room_id, r.name, r.topic"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(room_id, name, topic, num_joined_members)| PublicRoom {
            room_id,
            name,
            topic,
            num_joined_members,
        })
        .collect())
}

/// federation invite 用: ルームが存在しない場合のみプレースホルダーとして rooms テーブルに挿入する。
/// creator_user_id は NULL（federation 起源のルーム）。
pub async fn ensure_placeholder(pool: &MySqlPool, room_id: &str) -> Result<()> {
    sqlx::query("INSERT IGNORE INTO rooms (room_id, creator_user_id) VALUES (?, NULL)")
        .bind(room_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn invite(
    pool: &MySqlPool,
    room_id: &str,
    inviter_user_id: &str,
    invitee_user_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO room_memberships (room_id, user_id, membership, invited_by)
           VALUES (?, ?, 'invite', ?)
           ON DUPLICATE KEY UPDATE membership = 'invite', invited_by = VALUES(invited_by)"#,
    )
    .bind(room_id)
    .bind(invitee_user_id)
    .bind(inviter_user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// membership = 'invite' なルームを取得（sync の rooms.invite 用）
pub struct InvitedRoom {
    pub room_id: String,
    pub invited_by: Option<String>,
}

pub async fn invited_rooms(pool: &MySqlPool, user_id: &str) -> Result<Vec<InvitedRoom>> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT room_id, invited_by FROM room_memberships WHERE user_id = ? AND membership = 'invite'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(room_id, invited_by)| InvitedRoom {
            room_id,
            invited_by,
        })
        .collect())
}
