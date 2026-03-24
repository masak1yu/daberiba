/// サーバー公開鍵エンドポイント
/// GET /_matrix/key/v2/server
///
/// 自サーバーの署名検証キーを返す。
/// レスポンス自体をサーバーの秘密鍵で署名する（Matrix 仕様要件）。
use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/key/v2/server", get(get_server_keys))
}

async fn get_server_keys(State(state): State<AppState>) -> Json<serde_json::Value> {
    let server_name = &*state.server_name;

    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    // 24 時間有効
    let valid_until_ts = now_ms + 86_400_000;

    let pub_key = state.signing_key.public_key_base64();
    let key_id = &state.signing_key.key_id;

    // 署名対象オブジェクト（signatures フィールドなし）
    let mut obj = serde_json::json!({
        "server_name": server_name,
        "valid_until_ts": valid_until_ts,
        "verify_keys": {
            key_id: { "key": pub_key }
        },
        "old_verify_keys": {}
    });

    // カノニカル JSON に変換して署名
    let canonical = crate::signing_key::canonical_json(&obj);
    let sig = state.signing_key.sign(canonical.as_bytes());

    obj["signatures"] = serde_json::json!({
        server_name: {
            key_id: sig
        }
    });

    Json(obj)
}
