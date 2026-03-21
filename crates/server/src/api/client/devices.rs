use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/devices", get(list_devices))
        .route(
            "/_matrix/client/v3/devices/:device_id",
            get(get_device).put(update_device).delete(delete_device),
        )
        .route("/_matrix/client/v3/delete_devices", post(delete_devices))
}

async fn list_devices(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let devices = db::devices::list(&state.pool, &user.user_id).await?;
    let list: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            serde_json::json!({
                "device_id": d.device_id,
                "display_name": d.display_name,
                "last_seen_ts": d.last_seen_ts,
                "last_seen_ip": d.last_seen_ip,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "devices": list })))
}

async fn get_device(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(device_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let device = db::devices::get(&state.pool, &user.user_id, &device_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(serde_json::json!({
        "device_id": device.device_id,
        "display_name": device.display_name,
        "last_seen_ts": device.last_seen_ts,
        "last_seen_ip": device.last_seen_ip,
    })))
}

#[derive(Deserialize)]
struct UpdateDeviceBody {
    display_name: Option<String>,
}

async fn update_device(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(device_id): Path<String>,
    Json(body): Json<UpdateDeviceBody>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let found = db::devices::update_display_name(
        &state.pool,
        &user.user_id,
        &device_id,
        body.display_name.as_deref(),
    )
    .await?;
    if !found {
        return Err(AppError::NotFound);
    }
    Ok((StatusCode::OK, Json(serde_json::json!({}))))
}

async fn delete_device(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(device_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let found = db::devices::delete(&state.pool, &user.user_id, &device_id).await?;
    if !found {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct DeleteDevicesBody {
    devices: Vec<String>,
}

async fn delete_devices(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<DeleteDevicesBody>,
) -> ApiResult<Json<serde_json::Value>> {
    db::devices::delete_many(&state.pool, &user.user_id, &body.devices).await?;
    Ok(Json(serde_json::json!({})))
}
