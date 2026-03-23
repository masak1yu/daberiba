use anyhow::Result;
use sqlx::{MySqlPool, Row};

pub struct BackupVersion {
    pub id: u64,
    pub algorithm: String,
    pub auth_data: String,
    pub count: i64,
    pub etag: String,
}

/// 新しいバックアップバージョンを作成し、バージョン ID を返す
pub async fn create_version(
    pool: &MySqlPool,
    user_id: &str,
    algorithm: &str,
    auth_data: &str,
) -> Result<u64> {
    let result = sqlx::query(
        "INSERT INTO room_key_backup_versions (user_id, algorithm, auth_data) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(algorithm)
    .bind(auth_data)
    .execute(pool)
    .await?;
    Ok(result.last_insert_id())
}

/// バージョンを取得する。version = None なら最新の未削除バージョン
pub async fn get_version(
    pool: &MySqlPool,
    user_id: &str,
    version: Option<u64>,
) -> Result<Option<BackupVersion>> {
    let row = if let Some(v) = version {
        sqlx::query(
            r#"SELECT v.id, v.algorithm, v.auth_data,
                      COUNT(s.session_id) AS session_count
               FROM room_key_backup_versions v
               LEFT JOIN room_key_backup_sessions s ON s.version = v.id AND s.user_id = v.user_id
               WHERE v.user_id = ? AND v.id = ? AND v.deleted = 0
               GROUP BY v.id, v.algorithm, v.auth_data"#,
        )
        .bind(user_id)
        .bind(v)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query(
            r#"SELECT v.id, v.algorithm, v.auth_data,
                      COUNT(s.session_id) AS session_count
               FROM room_key_backup_versions v
               LEFT JOIN room_key_backup_sessions s ON s.version = v.id AND s.user_id = v.user_id
               WHERE v.user_id = ? AND v.deleted = 0
               GROUP BY v.id, v.algorithm, v.auth_data
               ORDER BY v.id DESC
               LIMIT 1"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
    };

    Ok(row.map(|r| {
        let id: u64 = r.get("id");
        let count: i64 = r.get("session_count");
        BackupVersion {
            id,
            algorithm: r.get("algorithm"),
            auth_data: r.get("auth_data"),
            count,
            etag: count.to_string(),
        }
    }))
}

/// バージョンの auth_data を更新する
pub async fn update_version(
    pool: &MySqlPool,
    user_id: &str,
    version: u64,
    auth_data: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE room_key_backup_versions SET auth_data = ? WHERE user_id = ? AND id = ? AND deleted = 0",
    )
    .bind(auth_data)
    .bind(user_id)
    .bind(version)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// バージョンを論理削除する（セッションも CASCADE で削除）
pub async fn delete_version(pool: &MySqlPool, user_id: &str, version: u64) -> Result<bool> {
    // セッションを物理削除してからバージョンを論理削除
    sqlx::query("DELETE FROM room_key_backup_sessions WHERE version = ? AND user_id = ?")
        .bind(version)
        .bind(user_id)
        .execute(pool)
        .await?;

    let result =
        sqlx::query("UPDATE room_key_backup_versions SET deleted = 1 WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(version)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub struct PutSessionArgs<'a> {
    pub user_id: &'a str,
    pub version: u64,
    pub room_id: &'a str,
    pub session_id: &'a str,
    pub first_message_index: i32,
    pub forwarded_count: i32,
    pub is_verified: bool,
    pub session_data: &'a str,
}

/// セッションキーを 1 件保存（upsert）。
/// first_message_index が小さい値を優先する。
pub async fn put_session(pool: &MySqlPool, args: PutSessionArgs<'_>) -> Result<()> {
    let PutSessionArgs {
        user_id,
        version,
        room_id,
        session_id,
        first_message_index,
        forwarded_count,
        is_verified,
        session_data,
    } = args;
    // 既存セッションより first_message_index が小さい場合のみ上書き
    sqlx::query(
        r#"INSERT INTO room_key_backup_sessions
               (version, user_id, room_id, session_id, first_message_index, forwarded_count, is_verified, session_data)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
               first_message_index = IF(VALUES(first_message_index) < first_message_index, VALUES(first_message_index), first_message_index),
               forwarded_count     = IF(VALUES(first_message_index) < first_message_index, VALUES(forwarded_count), forwarded_count),
               is_verified         = IF(VALUES(first_message_index) < first_message_index, VALUES(is_verified), is_verified),
               session_data        = IF(VALUES(first_message_index) < first_message_index, VALUES(session_data), session_data)"#,
    )
    .bind(version)
    .bind(user_id)
    .bind(room_id)
    .bind(session_id)
    .bind(first_message_index)
    .bind(forwarded_count)
    .bind(is_verified)
    .bind(session_data)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct SessionRow {
    pub room_id: String,
    pub session_id: String,
    pub first_message_index: i32,
    pub forwarded_count: i32,
    pub is_verified: bool,
    pub session_data: String,
}

/// セッションキーを取得する。room_id / session_id でフィルタ可能
pub async fn get_sessions(
    pool: &MySqlPool,
    user_id: &str,
    version: u64,
    room_id: Option<&str>,
    session_id: Option<&str>,
) -> Result<Vec<SessionRow>> {
    let rows = match (room_id, session_id) {
        (Some(rid), Some(sid)) => {
            sqlx::query(
                "SELECT room_id, session_id, first_message_index, forwarded_count, is_verified, session_data \
                 FROM room_key_backup_sessions WHERE version = ? AND user_id = ? AND room_id = ? AND session_id = ?",
            )
            .bind(version)
            .bind(user_id)
            .bind(rid)
            .bind(sid)
            .fetch_all(pool)
            .await?
        }
        (Some(rid), None) => {
            sqlx::query(
                "SELECT room_id, session_id, first_message_index, forwarded_count, is_verified, session_data \
                 FROM room_key_backup_sessions WHERE version = ? AND user_id = ? AND room_id = ?",
            )
            .bind(version)
            .bind(user_id)
            .bind(rid)
            .fetch_all(pool)
            .await?
        }
        _ => {
            sqlx::query(
                "SELECT room_id, session_id, first_message_index, forwarded_count, is_verified, session_data \
                 FROM room_key_backup_sessions WHERE version = ? AND user_id = ?",
            )
            .bind(version)
            .bind(user_id)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(rows
        .iter()
        .map(|r| {
            let verified: i8 = r.get("is_verified");
            SessionRow {
                room_id: r.get("room_id"),
                session_id: r.get("session_id"),
                first_message_index: r.get("first_message_index"),
                forwarded_count: r.get("forwarded_count"),
                is_verified: verified != 0,
                session_data: r.get("session_data"),
            }
        })
        .collect())
}

/// セッションを削除する。room_id / session_id でフィルタ可能
pub async fn delete_sessions(
    pool: &MySqlPool,
    user_id: &str,
    version: u64,
    room_id: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    match (room_id, session_id) {
        (Some(rid), Some(sid)) => {
            sqlx::query(
                "DELETE FROM room_key_backup_sessions WHERE version = ? AND user_id = ? AND room_id = ? AND session_id = ?",
            )
            .bind(version)
            .bind(user_id)
            .bind(rid)
            .bind(sid)
            .execute(pool)
            .await?;
        }
        (Some(rid), None) => {
            sqlx::query(
                "DELETE FROM room_key_backup_sessions WHERE version = ? AND user_id = ? AND room_id = ?",
            )
            .bind(version)
            .bind(user_id)
            .bind(rid)
            .execute(pool)
            .await?;
        }
        _ => {
            sqlx::query("DELETE FROM room_key_backup_sessions WHERE version = ? AND user_id = ?")
                .bind(version)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}
