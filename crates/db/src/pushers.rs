use anyhow::Result;
use sqlx::MySqlPool;

#[derive(sqlx::FromRow, Clone)]
pub struct Pusher {
    pub app_id: String,
    pub pushkey: String,
    pub user_id: String,
    pub kind: String,
    pub app_display_name: String,
    pub device_display_name: String,
    pub lang: String,
    /// JSON 文字列 {"url": "..."}
    pub data: String,
}

/// pusher を登録・更新する（app_id + pushkey でユニーク）。
pub async fn upsert(pool: &MySqlPool, p: &Pusher) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO pushers (app_id, pushkey, user_id, kind, app_display_name, device_display_name, lang, data)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
               user_id = VALUES(user_id),
               kind = VALUES(kind),
               app_display_name = VALUES(app_display_name),
               device_display_name = VALUES(device_display_name),
               lang = VALUES(lang),
               data = VALUES(data)"#,
        p.app_id,
        p.pushkey,
        p.user_id,
        p.kind,
        p.app_display_name,
        p.device_display_name,
        p.lang,
        p.data
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// pusher を削除する。
pub async fn delete(pool: &MySqlPool, user_id: &str, app_id: &str, pushkey: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM pushers WHERE app_id = ? AND pushkey = ? AND user_id = ?",
        app_id,
        pushkey,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// ユーザーの pusher 一覧を返す。
pub async fn list(pool: &MySqlPool, user_id: &str) -> Result<Vec<Pusher>> {
    let rows = sqlx::query_as!(
        Pusher,
        "SELECT app_id, pushkey, user_id, kind, app_display_name, device_display_name, lang, data \
         FROM pushers WHERE user_id = ?",
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// ルームメンバー（送信者を除く）の pusher を返す。イベント送信時の push 配送に使用。
pub async fn get_for_room_members(
    pool: &MySqlPool,
    room_id: &str,
    exclude_user_id: &str,
) -> Result<Vec<Pusher>> {
    let rows = sqlx::query_as!(
        Pusher,
        r#"SELECT p.app_id, p.pushkey, p.user_id, p.kind,
                  p.app_display_name, p.device_display_name, p.lang, p.data
           FROM pushers p
           JOIN room_memberships rm ON rm.user_id = p.user_id
           WHERE rm.room_id = ? AND rm.membership = 'join' AND p.user_id != ?"#,
        room_id,
        exclude_user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
