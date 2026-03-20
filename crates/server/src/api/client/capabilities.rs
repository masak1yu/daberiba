use axum::{routing::get, Json, Router};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/capabilities", get(capabilities))
}

async fn capabilities() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "capabilities": {
            "m.change_password": { "enabled": true },
            "m.room_versions": {
                "default": "10",
                "available": {
                    "10": "stable",
                }
            },
        }
    }))
}
