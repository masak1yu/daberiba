use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/matrix/client", get(well_known_client))
        .route("/.well-known/matrix/server", get(well_known_server))
}

async fn well_known_client(State(state): State<AppState>) -> Json<serde_json::Value> {
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| format!("http://{}:8448", state.server_name));

    Json(serde_json::json!({
        "m.homeserver": {
            "base_url": base_url,
        },
    }))
}

async fn well_known_server(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "m.server": format!("{}:8448", state.server_name),
    }))
}
