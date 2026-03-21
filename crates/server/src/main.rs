use anyhow::Result;
use dotenvy::dotenv;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod error;
mod media_store;
mod middleware;
mod router;
mod state;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = db::connect(&database_url).await?;

    let media_path = env::var("MEDIA_PATH").unwrap_or_else(|_| "./media".to_string());
    let media = std::sync::Arc::new(media_store::LocalStore::new(media_path).await?);

    let state = state::AppState::new(pool, media);
    let app = router::build(state);

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8448".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("listening on {}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
