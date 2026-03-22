use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/keys/upload", post(upload_keys))
        .route("/_matrix/client/v3/keys/query", post(query_keys))
        .route("/_matrix/client/v3/keys/claim", post(claim_keys))
}

#[derive(Deserialize)]
struct UploadKeysBody {
    device_keys: Option<serde_json::Value>,
    one_time_keys: Option<HashMap<String, serde_json::Value>>,
}

async fn upload_keys(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<UploadKeysBody>,
) -> ApiResult<Json<serde_json::Value>> {
    if let Some(device_keys) = body.device_keys {
        let key_json = serde_json::to_string(&device_keys).unwrap_or_default();
        db::keys::upload_device_keys(&state.pool, &user.user_id, &user.device_id, &key_json)
            .await?;
    }

    if let Some(one_time_keys) = body.one_time_keys {
        let pairs: Vec<(String, String)> = one_time_keys
            .into_iter()
            .map(|(k, v)| (k, serde_json::to_string(&v).unwrap_or_default()))
            .collect();
        db::keys::upload_one_time_keys(&state.pool, &user.user_id, &user.device_id, &pairs).await?;
    }

    let counts = db::keys::count_one_time_keys(&state.pool, &user.user_id, &user.device_id).await?;
    Ok(Json(serde_json::json!({ "one_time_key_counts": counts })))
}

#[derive(Deserialize)]
struct QueryKeysBody {
    device_keys: HashMap<String, Vec<String>>,
}

async fn query_keys(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Json(body): Json<QueryKeysBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut device_keys_map = serde_json::Map::new();

    for (user_id, device_ids) in body.device_keys {
        let rows = db::keys::get_device_keys(&state.pool, &user_id, &device_ids).await?;
        let mut user_devices = serde_json::Map::new();
        for (device_id, key_json) in rows {
            let v: serde_json::Value =
                serde_json::from_str(&key_json).unwrap_or(serde_json::Value::Null);
            user_devices.insert(device_id, v);
        }
        device_keys_map.insert(user_id, serde_json::Value::Object(user_devices));
    }

    Ok(Json(serde_json::json!({ "device_keys": device_keys_map })))
}

#[derive(Deserialize)]
struct ClaimKeysBody {
    one_time_keys: HashMap<String, HashMap<String, String>>,
}

async fn claim_keys(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Json(body): Json<ClaimKeysBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut result = serde_json::Map::new();

    for (user_id, devices) in body.one_time_keys {
        let mut user_map = serde_json::Map::new();
        for (device_id, algorithm) in devices {
            if let Some((key_id, key_json)) =
                db::keys::claim_one_time_key(&state.pool, &user_id, &device_id, &algorithm).await?
            {
                let v: serde_json::Value =
                    serde_json::from_str(&key_json).unwrap_or(serde_json::Value::Null);
                user_map.insert(key_id, v);
            }
        }
        result.insert(user_id, serde_json::Value::Object(user_map));
    }

    Ok(Json(serde_json::json!({ "one_time_keys": result })))
}
