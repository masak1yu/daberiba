use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/keys/upload", post(upload_keys))
        .route("/_matrix/client/v3/keys/query", post(query_keys))
        .route("/_matrix/client/v3/keys/claim", post(claim_keys))
        .route("/_matrix/client/v3/keys/changes", get(keys_changes))
        .route(
            "/_matrix/client/v3/keys/device_signing/upload",
            post(upload_device_signing),
        )
        .route(
            "/_matrix/client/v3/keys/signatures/upload",
            post(upload_signatures),
        )
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
    let mut master_keys_map = serde_json::Map::new();
    let mut self_signing_keys_map = serde_json::Map::new();
    let mut user_signing_keys_map = serde_json::Map::new();

    for (user_id, device_ids) in &body.device_keys {
        // デバイスキー
        let rows = db::keys::get_device_keys(&state.pool, user_id, device_ids).await?;
        let mut user_devices = serde_json::Map::new();
        for (device_id, key_json) in rows {
            let v: serde_json::Value =
                serde_json::from_str(&key_json).unwrap_or(serde_json::Value::Null);
            user_devices.insert(device_id, v);
        }
        device_keys_map.insert(user_id.clone(), serde_json::Value::Object(user_devices));

        // クロスサイニングキー
        let cs_keys = db::keys::get_cross_signing_keys(&state.pool, user_id).await?;
        for (key_type, key_json) in cs_keys {
            let v: serde_json::Value =
                serde_json::from_str(&key_json).unwrap_or(serde_json::Value::Null);
            match key_type.as_str() {
                "master" => {
                    master_keys_map.insert(user_id.clone(), v);
                }
                "self_signing" => {
                    self_signing_keys_map.insert(user_id.clone(), v);
                }
                "user_signing" => {
                    user_signing_keys_map.insert(user_id.clone(), v);
                }
                _ => {}
            }
        }
    }

    Ok(Json(serde_json::json!({
        "device_keys": device_keys_map,
        "master_keys": master_keys_map,
        "self_signing_keys": self_signing_keys_map,
        "user_signing_keys": user_signing_keys_map,
    })))
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

#[derive(Deserialize)]
struct KeysChangesQuery {
    from: Option<String>,
    #[allow(dead_code)]
    to: Option<String>,
}

/// GET /keys/changes — from/to トークン間でキーが変化したユーザー一覧を返す
async fn keys_changes(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<KeysChangesQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // from トークンから since_ms を抽出（sync トークン形式: ":ord_:td_id_:ms"）
    let since_ms = query.from.as_deref().and_then(|s| {
        let parts: Vec<&str> = s.splitn(3, '_').collect();
        parts.get(2).and_then(|v| v.parse::<u64>().ok())
    });

    let changed = db::keys::get_changed_users(&state.pool, &user.user_id, since_ms).await?;

    // left は from トークンの stream_ordering から
    let since_stream: u64 = query
        .from
        .as_deref()
        .and_then(|s| s.split('_').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let left = if since_stream > 0 {
        db::keys::get_left_users(&state.pool, &user.user_id, since_stream).await?
    } else {
        vec![]
    };

    Ok(Json(serde_json::json!({
        "changed": changed,
        "left": left,
    })))
}

#[derive(Deserialize)]
struct UploadDeviceSigningBody {
    master_key: Option<serde_json::Value>,
    self_signing_key: Option<serde_json::Value>,
    user_signing_key: Option<serde_json::Value>,
}

/// POST /keys/device_signing/upload — クロスサイニングキーをアップロード
async fn upload_device_signing(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<UploadDeviceSigningBody>,
) -> ApiResult<Json<serde_json::Value>> {
    if let Some(key) = body.master_key {
        let json = serde_json::to_string(&key).unwrap_or_default();
        db::keys::upload_cross_signing_keys(&state.pool, &user.user_id, "master", &json).await?;
    }
    if let Some(key) = body.self_signing_key {
        let json = serde_json::to_string(&key).unwrap_or_default();
        db::keys::upload_cross_signing_keys(&state.pool, &user.user_id, "self_signing", &json)
            .await?;
    }
    if let Some(key) = body.user_signing_key {
        let json = serde_json::to_string(&key).unwrap_or_default();
        db::keys::upload_cross_signing_keys(&state.pool, &user.user_id, "user_signing", &json)
            .await?;
    }
    Ok(Json(serde_json::json!({})))
}

/// POST /keys/signatures/upload — 署名をアップロード
/// Body: { "@user:server": { "key_id_or_device_id": { ...signed key object... } } }
async fn upload_signatures(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Json(body): Json<HashMap<String, HashMap<String, serde_json::Value>>>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut failures = serde_json::Map::new();

    for (target_user_id, keys) in &body {
        for (key_id, signed_key) in keys {
            let json = serde_json::to_string(signed_key).unwrap_or_default();
            if let Err(e) = db::keys::upload_key_signature(
                &state.pool,
                &user.user_id,
                target_user_id,
                key_id,
                &json,
            )
            .await
            {
                failures.insert(
                    format!("{target_user_id}/{key_id}"),
                    serde_json::json!({ "errcode": "M_UNKNOWN", "error": e.to_string() }),
                );
            }
        }
    }

    Ok(Json(serde_json::json!({ "failures": failures })))
}
