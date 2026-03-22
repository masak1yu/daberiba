use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/pushers", get(list_pushers))
        .route("/_matrix/client/v3/pushers/set", post(set_pusher))
}

async fn list_pushers(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let pushers = db::pushers::list(&state.pool, &user.user_id).await?;
    let list: Vec<serde_json::Value> = pushers
        .iter()
        .map(|p| {
            serde_json::json!({
                "app_id": p.app_id,
                "pushkey": p.pushkey,
                "kind": p.kind,
                "app_display_name": p.app_display_name,
                "device_display_name": p.device_display_name,
                "lang": p.lang,
                "data": serde_json::from_str::<serde_json::Value>(&p.data)
                    .unwrap_or_default(),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "pushers": list })))
}

#[derive(Deserialize)]
struct SetPusherBody {
    app_id: String,
    pushkey: String,
    /// null を指定すると pusher を削除する
    kind: Option<String>,
    app_display_name: Option<String>,
    device_display_name: Option<String>,
    lang: Option<String>,
    data: Option<serde_json::Value>,
}

async fn set_pusher(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<SetPusherBody>,
) -> ApiResult<Json<serde_json::Value>> {
    if body.kind.is_none() {
        // kind = null → 削除
        db::pushers::delete(&state.pool, &user.user_id, &body.app_id, &body.pushkey).await?;
        return Ok(Json(serde_json::json!({})));
    }

    let kind = body.kind.unwrap();
    let app_display_name = body
        .app_display_name
        .ok_or_else(|| AppError::BadRequest("app_display_name required".into()))?;
    let device_display_name = body
        .device_display_name
        .ok_or_else(|| AppError::BadRequest("device_display_name required".into()))?;
    let lang = body
        .lang
        .ok_or_else(|| AppError::BadRequest("lang required".into()))?;
    let data = body
        .data
        .ok_or_else(|| AppError::BadRequest("data required".into()))?;
    let data_str = serde_json::to_string(&data).map_err(|e| AppError::Internal(e.into()))?;

    let pusher = db::pushers::Pusher {
        app_id: body.app_id,
        pushkey: body.pushkey,
        user_id: user.user_id,
        kind,
        app_display_name,
        device_display_name,
        lang,
        data: data_str,
    };

    db::pushers::upsert(&state.pool, &pusher).await?;
    Ok(Json(serde_json::json!({})))
}
