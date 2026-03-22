use sqlx::{mysql::MySqlPoolOptions, MySqlPool};

pub mod access_tokens;
pub mod devices;
pub mod events;
pub mod media;
pub mod profile;
pub mod pushers;
pub mod room_state;
pub mod rooms;
pub mod sync;
pub mod users;

pub async fn connect(database_url: &str) -> anyhow::Result<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}
