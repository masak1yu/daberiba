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
        .merge(api::client::auth::protected_routes())
        .merge(api::client::account::routes())
        .merge(api::client::account_data::routes())
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
        .merge(api::client::keys::routes())
        .merge(api::client::push_rules::routes())
        .merge(api::client::room_keys::routes())
        .merge(api::client::public_rooms::routes())
        .merge(api::client::search::routes())
        .merge(api::client::notifications::routes())
        .merge(api::client::relations::routes())
        .merge(api::client::read_markers::routes())
        .merge(api::client::threepids::routes())
        .merge(api::client::threads::routes())
        .merge(api::client::timestamp_to_event::routes())
        .merge(api::client::hierarchy::routes())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // メディアルート（認証必須）
    let media =
        api::media::routes().layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Federation ルート（認証不要 — X-Matrix 検証は各ハンドラで行う）
    let federation = Router::new()
        .merge(api::federation::version::routes())
        .merge(api::federation::query::routes())
        .merge(api::federation::make_join::routes())
        .merge(api::federation::make_leave::routes())
        .merge(api::federation::send_join::routes())
        .merge(api::federation::send_leave::routes())
        .merge(api::federation::invite::routes())
        .merge(api::federation::send_transaction::routes())
        .merge(api::federation::get_event::routes())
        .merge(api::federation::backfill::routes());

    // サーバー公開鍵（認証不要）
    let server_keys = api::server_keys::routes();

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(media)
        .merge(federation)
        .merge(server_keys)
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
