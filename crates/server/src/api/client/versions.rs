use crate::state::AppState;
use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/versions", get(handler))
}

async fn handler() -> Json<Value> {
    Json(json!({
        "versions": ["v1.1", "v1.2", "v1.3", "v1.4", "v1.5"],
        "unstable_features": {}
    }))
}
