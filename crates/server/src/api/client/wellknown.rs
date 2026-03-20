use axum::{routing::get, Json, Router};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/matrix/client", get(well_known_client))
        .route("/.well-known/matrix/server", get(well_known_server))
}

async fn well_known_client() -> Json<serde_json::Value> {
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let base_url = std::env::var("BASE_URL")
        .unwrap_or_else(|_| format!("http://{}:8448", server_name));

    Json(serde_json::json!({
        "m.homeserver": {
            "base_url": base_url,
        },
    }))
}

async fn well_known_server() -> Json<serde_json::Value> {
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    Json(serde_json::json!({
        "m.server": format!("{}:8448", server_name),
    }))
}
