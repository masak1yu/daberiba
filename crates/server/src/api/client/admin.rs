use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Matrix 標準管理 API
        .route("/_matrix/client/v3/admin/whois/:userId", get(whois))
        // Synapse 互換管理 API
        .route("/_synapse/admin/v1/users", get(list_users))
        .route("/_synapse/admin/v1/users/:userId", get(get_user))
        .route(
            "/_synapse/admin/v1/deactivate/:userId",
            post(deactivate_user),
        )
        .route(
            "/_synapse/admin/v1/users/:userId/admin",
            put(set_user_admin),
        )
        .route("/_synapse/admin/v1/rooms", get(list_rooms))
        .route("/_synapse/admin/v1/media", get(list_media))
        .route(
            "/_synapse/admin/v1/media/:serverName/:mediaId",
            delete(delete_media),
        )
        .route("/_synapse/admin/v1/event_reports", get(list_event_reports))
}

/// 管理者チェック: 呼び出し元が admin でない場合は 403 を返す。
async fn require_admin(
    pool: &sqlx::MySqlPool,
    user_id: &str,
) -> Result<(), crate::error::AppError> {
    if !db::users::is_admin(pool, user_id).await.unwrap_or(false) {
        return Err(crate::error::AppError::Forbidden);
    }
    Ok(())
}

/// GET /_matrix/client/v3/admin/whois/:userId
/// ユーザーのセッション情報（デバイス・アクセストークン）を返す（Matrix 標準）。
async fn whois(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(target_user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // 自分自身か管理者のみ取得可能
    if user.user_id != target_user_id {
        require_admin(&state.pool, &user.user_id).await?;
    }

    let devices = db::devices::list(&state.pool, &target_user_id)
        .await
        .unwrap_or_default();

    // デバイスごとのセッション情報
    let mut devices_map = serde_json::Map::new();
    for dev in &devices {
        let sessions = serde_json::json!([{
            "last_seen": dev.last_seen_ts,
            "ip": dev.last_seen_ip,
            "display_name": dev.display_name,
        }]);
        devices_map.insert(
            dev.device_id.clone(),
            serde_json::json!({ "sessions": sessions }),
        );
    }

    Ok(Json(serde_json::json!({
        "user_id": target_user_id,
        "devices": devices_map,
    })))
}

#[derive(Deserialize, Default)]
struct ListUsersQuery {
    from: Option<u64>,
    limit: Option<u64>,
}

/// GET /_synapse/admin/v1/users
/// 全ユーザー一覧を返す。管理者専用。
async fn list_users(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<ListUsersQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.pool, &user.user_id).await?;

    let limit = query.limit.unwrap_or(100).min(500) as usize;
    let from = query.from.unwrap_or(0) as usize;

    let mut all_users = db::users::list_all(&state.pool).await?;
    let total = all_users.len();
    let page: Vec<_> = all_users.drain(from..).take(limit).collect();
    let next_token = if from + limit < total {
        Some((from + limit) as u64)
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "users": page,
        "next_token": next_token,
        "total": total,
    })))
}

/// GET /_synapse/admin/v1/users/:userId
/// 特定ユーザーの詳細を返す。管理者専用。
async fn get_user(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(target_user_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.pool, &user.user_id).await?;

    let all_users = db::users::list_all(&state.pool).await?;
    let target = all_users
        .into_iter()
        .find(|u| u.get("user_id").and_then(|v| v.as_str()) == Some(&target_user_id))
        .ok_or(crate::error::AppError::NotFound)?;

    Ok(Json(target))
}

/// POST /_synapse/admin/v1/deactivate/:userId
/// ユーザーを無効化する。管理者専用。
async fn deactivate_user(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(target_user_id): Path<String>,
) -> ApiResult<StatusCode> {
    require_admin(&state.pool, &user.user_id).await?;

    db::users::admin_deactivate(&state.pool, &target_user_id).await?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize, Default)]
struct ListRoomsQuery {
    from: Option<u64>,
    limit: Option<u64>,
}

/// GET /_synapse/admin/v1/rooms
/// 全ルーム一覧を返す。管理者専用。
async fn list_rooms(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<ListRoomsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.pool, &user.user_id).await?;

    let limit = query.limit.unwrap_or(100).min(500) as usize;
    let from = query.from.unwrap_or(0) as usize;

    let mut all_rooms = db::rooms::list_all(&state.pool).await?;
    let total = all_rooms.len();
    let page: Vec<_> = all_rooms.drain(from..).take(limit).collect();
    let next_token = if from + limit < total {
        Some((from + limit) as u64)
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "rooms": page,
        "next_token": next_token,
        "total": total,
    })))
}

#[derive(Deserialize)]
struct SetAdminBody {
    admin: bool,
}

/// PUT /_synapse/admin/v1/users/:userId/admin
/// ユーザーの管理者フラグを設定する。管理者専用。
async fn set_user_admin(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(target_user_id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> ApiResult<StatusCode> {
    require_admin(&state.pool, &user.user_id).await?;

    if !db::users::exists(&state.pool, &target_user_id).await? {
        return Err(crate::error::AppError::NotFound);
    }
    db::users::set_admin(&state.pool, &target_user_id, body.admin).await?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize, Default)]
struct ListMediaQuery {
    from: Option<u64>,
    limit: Option<u64>,
}

/// GET /_synapse/admin/v1/media
/// 全メディア一覧を返す。管理者専用。
async fn list_media(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<ListMediaQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.pool, &user.user_id).await?;

    let limit = query.limit.unwrap_or(100).min(500) as usize;
    let from = query.from.unwrap_or(0) as usize;

    let mut all_media = db::media::list_all(&state.pool).await?;
    let total = all_media.len();
    let page: Vec<_> = all_media
        .drain(from..)
        .take(limit)
        .map(|m| {
            serde_json::json!({
                "media_id": m.media_id,
                "server_name": m.server_name,
                "user_id": m.user_id,
                "content_type": m.content_type,
                "filename": m.filename,
                "file_size": m.file_size,
                "room_id": m.room_id,
                "mxc_uri": format!("mxc://{}/{}", m.server_name, m.media_id),
            })
        })
        .collect();
    let next_token = if from + limit < total {
        Some((from + limit) as u64)
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "media": page,
        "next_token": next_token,
        "total": total,
    })))
}

/// DELETE /_synapse/admin/v1/media/:serverName/:mediaId
/// メディアを削除する（DB レコード + ストレージ両方）。管理者専用。
async fn delete_media(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    require_admin(&state.pool, &user.user_id).await?;

    let deleted = db::media::delete(&state.pool, &server_name, &media_id).await?;
    if !deleted {
        return Err(crate::error::AppError::NotFound);
    }

    // ストレージからも削除（ローカル or S3）
    let _ = state.media.delete(&media_id).await;

    Ok(StatusCode::OK)
}

/// GET /_synapse/admin/v1/event_reports
/// コンテンツ報告一覧を返す。管理者専用。
async fn list_event_reports(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&state.pool, &user.user_id).await?;
    let reports = db::reports::list_all(&state.pool).await?;
    let total = reports.len();
    Ok(Json(serde_json::json!({
        "event_reports": reports,
        "total": total,
    })))
}
