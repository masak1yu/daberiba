use crate::state::AppState;
use axum::{routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/federation/v1/version", get(get_version))
}

async fn get_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "server": {
            "name": "daberiba",
            "version": env!("CARGO_PKG_VERSION"),
        }
    }))
}
