use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v3/sendToDevice/:eventType/:txnId",
        put(send_to_device),
    )
}

#[derive(Deserialize)]
struct SendToDevicePath {
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "txnId")]
    _txn_id: String,
}

/// { "messages": { "@user:server": { "device_id": { ...content... } } } }
#[derive(Deserialize)]
struct SendToDeviceBody {
    messages: HashMap<String, HashMap<String, serde_json::Value>>,
}

async fn send_to_device(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(path): Path<SendToDevicePath>,
    Json(body): Json<SendToDeviceBody>,
) -> ApiResult<StatusCode> {
    for (recipient, devices) in &body.messages {
        for (device_id, content) in devices {
            let content_str = serde_json::to_string(content).unwrap_or_default();
            if device_id == "*" {
                // 全デバイス宛て: 受信者の全デバイスに個別に挿入する
                let all_devices = db::devices::list(&state.pool, recipient)
                    .await
                    .unwrap_or_default();
                for dev in &all_devices {
                    db::to_device::send(
                        &state.pool,
                        &user.user_id,
                        recipient,
                        &dev.device_id,
                        &path.event_type,
                        &content_str,
                        "",
                    )
                    .await?;
                }
            } else {
                db::to_device::send(
                    &state.pool,
                    &user.user_id,
                    recipient,
                    device_id,
                    &path.event_type,
                    &content_str,
                    "",
                )
                .await?;
            }
        }
    }
    Ok(StatusCode::OK)
}
