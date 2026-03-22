use crate::{api, middleware::auth::require_auth, state::AppState};
use axum::{http::HeaderValue, middleware, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn build(state: AppState) -> Router {
    // 認証不要ルート
    let public = Router::new()
        .merge(api::client::versions::routes())
        .merge(api::client::auth::routes())
        .merge(api::client::capabilities::routes())
        .merge(api::client::wellknown::routes());

    // 認証必須ルート
    let protected = Router::new()
        .merge(api::client::account::routes())
        .merge(api::client::filters::routes())
        .merge(api::client::room_tags::routes())
        .merge(api::client::devices::routes())
        .merge(api::client::rooms::routes())
        .merge(api::client::room_state::routes())
        .merge(api::client::events::routes())
        .merge(api::client::presence::routes())
        .merge(api::client::profile::routes())
        .merge(api::client::sync::routes())
        .merge(api::client::pushers::routes())
        .merge(api::client::receipts::routes())
        .merge(api::client::room_aliases::routes())
        .merge(api::client::to_device::routes())
        .merge(api::client::typing_notif::routes())
        .merge(api::client::public_rooms::routes())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // メディアルート（認証必須）
    let media =
        api::media::routes().layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(media)
        .layer(build_cors())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn build_cors() -> CorsLayer {
    // CORS_ORIGINS=* で全許可、カンマ区切りで複数指定可
    // 例: CORS_ORIGINS=https://app.example.com,https://dev.example.com
    let origins_env = std::env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string());

    if origins_env == "*" {
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = origins_env
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
}
