use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/_matrix/client/v3/directory/room/{roomAlias}",
            put(put_alias).get(get_alias).delete(delete_alias),
        )
        .route(
            "/_matrix/client/v3/rooms/{roomId}/aliases",
            get(list_room_aliases),
        )
}

#[derive(Deserialize)]
struct AliasPutBody {
    room_id: String,
}

#[derive(Serialize)]
struct AliasGetResponse {
    room_id: String,
    servers: Vec<String>,
}

async fn put_alias(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(alias): Path<String>,
    Json(body): Json<AliasPutBody>,
) -> ApiResult<StatusCode> {
    match db::room_aliases::create(&state.pool, &alias, &body.room_id, &user.user_id).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            // 重複エイリアス（ユニーク制約違反）
            if e.to_string().contains("Duplicate entry") || e.to_string().contains("1062") {
                Err(AppError::BadRequest(
                    "M_UNKNOWN: alias already exists".into(),
                ))
            } else {
                Err(AppError::Internal(e))
            }
        }
    }
}

async fn get_alias(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> ApiResult<Json<AliasGetResponse>> {
    let room_id = db::room_aliases::resolve(&state.pool, &alias)
        .await?
        .ok_or(AppError::NotFound)?;

    // server_name を room_id から抽出（!opaque:server_name 形式）
    let server = room_id
        .split_once(':')
        .map(|x| x.1)
        .unwrap_or("localhost")
        .to_string();

    Ok(Json(AliasGetResponse {
        room_id,
        servers: vec![server],
    }))
}

async fn delete_alias(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(alias): Path<String>,
) -> ApiResult<StatusCode> {
    let creator = db::room_aliases::get_creator(&state.pool, &alias)
        .await?
        .ok_or(AppError::NotFound)?;

    if creator != user.user_id {
        return Err(AppError::Forbidden);
    }

    db::room_aliases::delete(&state.pool, &alias).await?;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct RoomAliasesResponse {
    aliases: Vec<String>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/aliases
/// ルームに紐づくエイリアス一覧を返す。
/// room_aliases テーブルのエイリアスに加えて m.room.canonical_alias の
/// alias / alt_aliases も含める（重複は除く）。
async fn list_room_aliases(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<RoomAliasesResponse>> {
    let mut aliases = db::room_aliases::list_for_room(&state.pool, &room_id).await?;

    // m.room.canonical_alias 状態イベントから alias / alt_aliases も追加
    if let Ok(Some(content)) =
        db::room_state::get_event(&state.pool, &room_id, "m.room.canonical_alias", "").await
    {
        if let Some(alias) = content.get("alias").and_then(|v| v.as_str()) {
            if !aliases.contains(&alias.to_string()) {
                aliases.push(alias.to_string());
            }
        }
        if let Some(alt) = content.get("alt_aliases").and_then(|v| v.as_array()) {
            for a in alt {
                if let Some(s) = a.as_str() {
                    if !aliases.contains(&s.to_string()) {
                        aliases.push(s.to_string());
                    }
                }
            }
        }
    }

    Ok(Json(RoomAliasesResponse { aliases }))
}
