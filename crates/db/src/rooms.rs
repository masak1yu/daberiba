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

/// membership / not_membership フィルタ付きメンバー一覧を返す。
/// どちらも None の場合は全メンバーを返す（既存 get_members 相当）。
pub async fn get_members_filtered(
    pool: &MySqlPool,
    room_id: &str,
    membership: Option<&str>,
    not_membership: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    // 動的 WHERE 句を構築する。
    let mut conditions = vec!["rm.room_id = ?"];
    if membership.is_some() {
        conditions.push("rm.membership = ?");
    }
    if not_membership.is_some() {
        conditions.push("rm.membership != ?");
    }

    let sql = format!(
        r#"SELECT rm.user_id, rm.membership, u.display_name, u.avatar_url
           FROM room_memberships rm
           JOIN users u ON u.user_id = rm.user_id
           WHERE {}"#,
        conditions.join(" AND ")
    );

    let mut q = sqlx::query(&sql).bind(room_id);
    if let Some(m) = membership {
        q = q.bind(m);
    }
    if let Some(nm) = not_membership {
        q = q.bind(nm);
    }

    let rows = q.fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let user_id: String = r.get("user_id");
            let membership: String = r.get("membership");
            let display_name: Option<String> = r.get("display_name");
            let avatar_url: Option<String> = r.get("avatar_url");
            serde_json::json!({
                "type": "m.room.member",
                "state_key": user_id,
                "content": {
                    "membership": membership,
                    "displayname": display_name,
                    "avatar_url": avatar_url,
                },
            })
        })
        .collect())
}

/// 指定 stream_ordering 時点でのメンバースナップショットを返す。
/// events テーブルから m.room.member イベントを再構築し、
/// membership / not_membership フィルタを Rust 側で適用する。
pub async fn get_members_at(
    pool: &MySqlPool,
    room_id: &str,
    at_ordering: u64,
    membership: Option<&str>,
    not_membership: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    // 各 state_key について stream_ordering <= at_ordering の最新イベントを取得する。
    let rows = sqlx::query(
        r#"SELECT e.state_key, e.content
           FROM events e
           WHERE e.room_id = ?
             AND e.event_type = 'm.room.member'
             AND e.stream_ordering = (
                 SELECT MAX(e2.stream_ordering)
                 FROM events e2
                 WHERE e2.room_id = e.room_id
                   AND e2.event_type = 'm.room.member'
                   AND e2.state_key = e.state_key
                   AND e2.stream_ordering <= ?
             )"#,
    )
    .bind(room_id)
    .bind(at_ordering)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let user_id: String = row.get("state_key");
        let content_str: String = row.get("content");
        let content: serde_json::Value = serde_json::from_str(&content_str).unwrap_or_default();

        let mem = content
            .get("membership")
            .and_then(|v| v.as_str())
            .unwrap_or("leave");

        if let Some(m) = membership {
            if mem != m {
                continue;
            }
        }
        if let Some(nm) = not_membership {
            if mem == nm {
                continue;
            }
        }

        result.push(serde_json::json!({
            "type": "m.room.member",
            "state_key": user_id,
            "content": content,
        }));
    }

    Ok(result)
}

/// 指定ユーザー群の現在の m.room.member イベントを返す（lazy_load_members 用）。
pub async fn get_member_events_for_users(
    pool: &MySqlPool,
    room_id: &str,
    user_ids: &[String],
) -> Result<Vec<serde_json::Value>> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    use sqlx::Row;

    // IN 句を動的に組み立てる。
    let placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        r#"SELECT e.event_id, e.sender, e.state_key, e.content, e.created_at
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ?
             AND rs.event_type = 'm.room.member'
             AND rs.state_key IN ({placeholders})"#
    );

    let mut q = sqlx::query(&sql).bind(room_id);
    for uid in user_ids {
        q = q.bind(uid);
    }

    let rows = q.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let content_str: String = r.get("content");
            let ts: chrono::NaiveDateTime = r.get("created_at");
            serde_json::json!({
                "event_id": r.get::<String, _>("event_id"),
                "sender": r.get::<String, _>("sender"),
                "type": "m.room.member",
                "state_key": r.get::<String, _>("state_key"),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
                "origin_server_ts": ts.and_utc().timestamp_millis(),
                "room_id": room_id,
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

/// ルーム内で join 中の外部サーバー名一覧を返す（自サーバーを除く）。
/// federation send_transaction の送信先決定に使用する。
pub async fn remote_servers_in_room(
    pool: &MySqlPool,
    room_id: &str,
    local_server: &str,
) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT DISTINCT SUBSTRING_INDEX(user_id, ':', -1) AS server
           FROM room_memberships
           WHERE room_id = ? AND membership = 'join'
             AND user_id NOT LIKE ?"#,
    )
    .bind(room_id)
    .bind(format!("%:{}", local_server))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(s,)| s).collect())
}

/// since_ordering より後に leave になったルームの room_id 一覧を返す（sync の rooms.leave 用）。
/// membership = 'leave' で、かつ leave イベント（m.room.member / state_key = user_id）が
/// since_ordering より後に存在するルームを返す。
pub async fn leave_rooms_since(
    pool: &MySqlPool,
    user_id: &str,
    since_ordering: u64,
) -> Result<Vec<String>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT DISTINCT rm.room_id
           FROM room_memberships rm
           INNER JOIN events e ON e.room_id = rm.room_id
             AND e.event_type = 'm.room.member'
             AND e.state_key = ?
             AND e.stream_ordering > ?
             AND e.content LIKE '%"membership":"leave"%'
           WHERE rm.user_id = ? AND rm.membership = 'leave'"#,
    )
    .bind(user_id)
    .bind(since_ordering)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.get("room_id")).collect())
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

/// membership を 'ban' に設定する。
pub async fn ban(pool: &MySqlPool, room_id: &str, user_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO room_memberships (room_id, user_id, membership)
           VALUES (?, ?, 'ban')
           ON DUPLICATE KEY UPDATE membership = 'ban'"#,
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// ban を解除して membership = 'leave' に戻す（再招待可能状態）。
pub async fn unban(pool: &MySqlPool, room_id: &str, user_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE room_memberships SET membership = 'leave' WHERE room_id = ? AND user_id = ? AND membership = 'ban'",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// ルームの記録をユーザーの視点から削除する（forget）。
/// leave / ban 状態のユーザーのみ実行可能（join 中は削除しない）。
pub async fn forget(pool: &MySqlPool, room_id: &str, user_id: &str) -> Result<()> {
    sqlx::query(
        "DELETE FROM room_memberships WHERE room_id = ? AND user_id = ? AND membership != 'join'",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 全ルーム一覧を返す（管理者向け）。joined_members 数と creator を含む。
pub async fn list_all(pool: &MySqlPool) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT r.room_id, r.name, r.topic, r.creator_user_id, r.created_at,
                  COUNT(m.user_id) AS joined_members
           FROM rooms r
           LEFT JOIN room_memberships m ON m.room_id = r.room_id AND m.membership = 'join'
           GROUP BY r.room_id, r.name, r.topic, r.creator_user_id, r.created_at
           ORDER BY r.created_at ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let created_at: chrono::NaiveDateTime = r.get("created_at");
            serde_json::json!({
                "room_id": r.get::<String, _>("room_id"),
                "name": r.get::<Option<String>, _>("name"),
                "topic": r.get::<Option<String>, _>("topic"),
                "creator": r.get::<Option<String>, _>("creator_user_id"),
                "joined_members": r.get::<i64, _>("joined_members"),
                "creation_ts": created_at.and_utc().timestamp_millis(),
            })
        })
        .collect())
}
