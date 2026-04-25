use anyhow::Result;
use dotenvy::dotenv;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod error;
mod federation_client;
mod filter;
mod media_store;
mod middleware;
mod push_eval;
mod router;
mod signing_key;
mod sso;
mod state;
mod state_res;
mod typing_store;
mod uia;
mod xmatrix;

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

    // DATABASE_URL が未設定の場合は DB_* 変数から構築する
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        let host = env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
        let user = env::var("DB_USER").expect("DATABASE_URL or DB_USER must be set");
        let pass = env::var("DB_PASS").expect("DATABASE_URL or DB_PASS must be set");
        let name = env::var("DB_NAME").unwrap_or_else(|_| user.clone());
        format!("mysql://{user}:{pass}@{host}:{port}/{name}")
    });

    let pool = db::connect(&database_url).await?;

    let media: std::sync::Arc<dyn media_store::MediaStore> =
        match env::var("MEDIA_BACKEND").as_deref() {
            #[cfg(feature = "s3")]
            Ok("s3") => {
                let bucket =
                    env::var("S3_BUCKET").expect("S3_BUCKET must be set when MEDIA_BACKEND=s3");
                std::sync::Arc::new(media_store::S3Store::new(bucket).await?)
            }
            _ => {
                let media_path = env::var("MEDIA_PATH").unwrap_or_else(|_| "./media".to_string());
                std::sync::Arc::new(media_store::LocalStore::new(media_path).await?)
            }
        };

    let state = state::AppState::new(pool, media).await;
    let app = router::build(state);

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8448".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("listening on {}", bind_addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
