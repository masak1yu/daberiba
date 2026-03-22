use sqlx::{mysql::MySqlPoolOptions, MySqlPool};

pub mod access_tokens;
pub mod devices;
pub mod events;
pub mod filters;
pub mod keys;
pub mod media;
pub mod presence;
pub mod profile;
pub mod pushers;
pub mod receipts;
pub mod room_aliases;
pub mod room_state;
pub mod room_tags;
pub mod rooms;
pub mod sync;
pub mod to_device;
pub mod unread;
pub mod users;

pub async fn connect(database_url: &str) -> anyhow::Result<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}
