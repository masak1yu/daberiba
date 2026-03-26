use sqlx::{mysql::MySqlPoolOptions, MySqlPool};

pub mod access_tokens;
pub mod account_data;
pub mod devices;
pub mod events;
pub mod filters;
pub mod keys;
pub mod login_tokens;
pub mod media;
pub mod notifications;
pub mod presence;
pub mod profile;
pub mod pushers;
pub mod receipts;
pub mod relations;
pub mod reports;
pub mod room_aliases;
pub mod room_keys;
pub mod room_state;
pub mod room_tags;
pub mod rooms;
pub mod server_signing_key;
pub mod sync;
pub mod threepids;
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
