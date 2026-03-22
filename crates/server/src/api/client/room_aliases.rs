use crate::{error::ApiResult, error::AppError, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/directory/room/{roomAlias}",
        put(put_alias).get(get_alias).delete(delete_alias),
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
        .splitn(2, ':')
        .nth(1)
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
