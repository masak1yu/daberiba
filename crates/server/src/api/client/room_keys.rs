use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        // バージョン管理
        .route(
            "/_matrix/client/v3/room_keys/version",
            post(create_version).get(get_latest_version),
        )
        .route(
            "/_matrix/client/v3/room_keys/version/{version}",
            get(get_version).put(put_version).delete(delete_version),
        )
        // 全キー操作
        .route(
            "/_matrix/client/v3/room_keys/keys",
            put(put_keys).get(get_keys).delete(del_keys),
        )
        // ルーム単位のキー操作
        .route(
            "/_matrix/client/v3/room_keys/keys/{roomId}",
            put(put_room_keys).get(get_room_keys).delete(del_room_keys),
        )
        // セッション単位のキー操作
        .route(
            "/_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}",
            put(put_session_key)
                .get(get_session_key)
                .delete(del_session_key),
        )
}

#[derive(Deserialize)]
struct VersionQuery {
    version: Option<String>,
}

#[derive(Deserialize)]
struct VersionBody {
    algorithm: String,
    auth_data: serde_json::Value,
}

fn version_response(v: db::room_keys::BackupVersion) -> serde_json::Value {
    serde_json::json!({
        "algorithm": v.algorithm,
        "auth_data": serde_json::from_str::<serde_json::Value>(&v.auth_data).unwrap_or_default(),
        "count": v.count,
        "etag": v.etag,
        "version": v.id.to_string(),
    })
}

async fn create_version(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<VersionBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let auth_data = serde_json::to_string(&body.auth_data)
        .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
    let id = db::room_keys::create_version(&state.pool, &user.user_id, &body.algorithm, &auth_data)
        .await?;
    Ok(Json(serde_json::json!({ "version": id.to_string() })))
}

async fn get_latest_version(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let v = db::room_keys::get_version(&state.pool, &user.user_id, None)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(version_response(v)))
}

async fn get_version(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(version): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid: u64 = version
        .parse()
        .map_err(|_| crate::error::AppError::NotFound)?;
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(version_response(v)))
}

async fn put_version(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(version): Path<String>,
    Json(body): Json<VersionBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid: u64 = version
        .parse()
        .map_err(|_| crate::error::AppError::NotFound)?;
    let auth_data = serde_json::to_string(&body.auth_data)
        .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
    let found = db::room_keys::update_version(&state.pool, &user.user_id, vid, &auth_data).await?;
    if !found {
        return Err(crate::error::AppError::NotFound);
    }
    Ok(Json(serde_json::json!({})))
}

async fn delete_version(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(version): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid: u64 = version
        .parse()
        .map_err(|_| crate::error::AppError::NotFound)?;
    let found = db::room_keys::delete_version(&state.pool, &user.user_id, vid).await?;
    if !found {
        return Err(crate::error::AppError::NotFound);
    }
    Ok(Json(serde_json::json!({})))
}

// ─── キー操作共通ヘルパー ──────────────────────────────────────────────────────

/// ?version= クエリから版番号を取得（必須）
fn parse_version_param(v: Option<String>) -> ApiResult<u64> {
    v.as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or(crate::error::AppError::BadRequest(
            "version query parameter is required".into(),
        ))
}

/// セッションデータを JSON から解析する
#[derive(Deserialize)]
struct SessionKeyBody {
    first_message_index: i32,
    forwarded_count: i32,
    is_verified: bool,
    session_data: serde_json::Value,
}

fn sessions_to_json(sessions: Vec<db::room_keys::SessionRow>) -> serde_json::Value {
    let mut by_room: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> =
        std::collections::HashMap::new();
    for s in sessions {
        let session_data: serde_json::Value =
            serde_json::from_str(&s.session_data).unwrap_or_default();
        by_room.entry(s.room_id).or_default().insert(
            s.session_id,
            serde_json::json!({
                "first_message_index": s.first_message_index,
                "forwarded_count": s.forwarded_count,
                "is_verified": s.is_verified,
                "session_data": session_data,
            }),
        );
    }
    let rooms: serde_json::Map<String, serde_json::Value> = by_room
        .into_iter()
        .map(|(room_id, sessions)| (room_id, serde_json::json!({ "sessions": sessions })))
        .collect();
    serde_json::json!({ "rooms": rooms })
}

// ─── 全キー ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AllKeysBody {
    rooms: std::collections::HashMap<String, RoomKeysBody>,
}

#[derive(Deserialize)]
struct RoomKeysBody {
    sessions: std::collections::HashMap<String, SessionKeyBody>,
}

async fn put_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(q): Query<VersionQuery>,
    Json(body): Json<AllKeysBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    for (room_id, room) in &body.rooms {
        for (session_id, s) in &room.sessions {
            let data = serde_json::to_string(&s.session_data)
                .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
            db::room_keys::put_session(
                &state.pool,
                db::room_keys::PutSessionArgs {
                    user_id: &user.user_id,
                    version: vid,
                    room_id,
                    session_id,
                    first_message_index: s.first_message_index,
                    forwarded_count: s.forwarded_count,
                    is_verified: s.is_verified,
                    session_data: &data,
                },
            )
            .await?;
        }
    }
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}

async fn get_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    let sessions = db::room_keys::get_sessions(&state.pool, &user.user_id, vid, None, None).await?;
    Ok(Json(sessions_to_json(sessions)))
}

async fn del_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    db::room_keys::delete_sessions(&state.pool, &user.user_id, vid, None, None).await?;
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}

// ─── ルーム単位のキー ─────────────────────────────────────────────────────────

async fn put_room_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(q): Query<VersionQuery>,
    Json(body): Json<RoomKeysBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    for (session_id, s) in &body.sessions {
        let data = serde_json::to_string(&s.session_data)
            .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
        db::room_keys::put_session(
            &state.pool,
            db::room_keys::PutSessionArgs {
                user_id: &user.user_id,
                version: vid,
                room_id: &room_id,
                session_id,
                first_message_index: s.first_message_index,
                forwarded_count: s.forwarded_count,
                is_verified: s.is_verified,
                session_data: &data,
            },
        )
        .await?;
    }
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}

async fn get_room_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    let sessions =
        db::room_keys::get_sessions(&state.pool, &user.user_id, vid, Some(&room_id), None).await?;
    // room レベルのレスポンス形式: { "sessions": { sessionId: {...} } }
    let mut sess_map = serde_json::Map::new();
    for s in sessions {
        let session_data: serde_json::Value =
            serde_json::from_str(&s.session_data).unwrap_or_default();
        sess_map.insert(
            s.session_id,
            serde_json::json!({
                "first_message_index": s.first_message_index,
                "forwarded_count": s.forwarded_count,
                "is_verified": s.is_verified,
                "session_data": session_data,
            }),
        );
    }
    Ok(Json(serde_json::json!({ "sessions": sess_map })))
}

async fn del_room_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    db::room_keys::delete_sessions(&state.pool, &user.user_id, vid, Some(&room_id), None).await?;
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}

// ─── セッション単位のキー ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RoomSessionPath {
    #[serde(rename = "roomId")]
    room_id: String,
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn put_session_key(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<RoomSessionPath>,
    Query(q): Query<VersionQuery>,
    Json(body): Json<SessionKeyBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    let data = serde_json::to_string(&body.session_data)
        .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
    db::room_keys::put_session(
        &state.pool,
        db::room_keys::PutSessionArgs {
            user_id: &user.user_id,
            version: vid,
            room_id: &path.room_id,
            session_id: &path.session_id,
            first_message_index: body.first_message_index,
            forwarded_count: body.forwarded_count,
            is_verified: body.is_verified,
            session_data: &data,
        },
    )
    .await?;
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}

async fn get_session_key(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<RoomSessionPath>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    let mut sessions = db::room_keys::get_sessions(
        &state.pool,
        &user.user_id,
        vid,
        Some(&path.room_id),
        Some(&path.session_id),
    )
    .await?;
    let s = sessions.pop().ok_or(crate::error::AppError::NotFound)?;
    let session_data: serde_json::Value = serde_json::from_str(&s.session_data).unwrap_or_default();
    Ok(Json(serde_json::json!({
        "first_message_index": s.first_message_index,
        "forwarded_count": s.forwarded_count,
        "is_verified": s.is_verified,
        "session_data": session_data,
    })))
}

async fn del_session_key(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<RoomSessionPath>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = parse_version_param(q.version)?;
    db::room_keys::delete_sessions(
        &state.pool,
        &user.user_id,
        vid,
        Some(&path.room_id),
        Some(&path.session_id),
    )
    .await?;
    let v = db::room_keys::get_version(&state.pool, &user.user_id, Some(vid))
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(
        serde_json::json!({ "count": v.count, "etag": v.etag }),
    ))
}
