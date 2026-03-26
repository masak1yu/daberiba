use crate::state::AppState;
use axum::{routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v3/capabilities", get(capabilities))
}

async fn capabilities() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "capabilities": {
            "m.change_password": { "enabled": true },
            "m.set_displayname": { "enabled": true },
            "m.set_avatar_url": { "enabled": true },
            "m.3pid_changes": { "enabled": false },
            "m.get_login_token": { "enabled": false },
            "m.room_versions": {
                "default": "10",
                "available": {
                    "9": "stable",
                    "10": "stable",
                    "11": "stable",
                }
            },
        }
    }))
}
