use axum::{middleware, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use crate::{api, middleware::auth::require_auth, state::AppState};

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
        .merge(api::client::rooms::routes())
        .merge(api::client::room_state::routes())
        .merge(api::client::events::routes())
        .merge(api::client::profile::routes())
        .merge(api::client::sync::routes())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
