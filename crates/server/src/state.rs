use crate::media_store::MediaStore;
use crate::uia::UiaStore;
use sqlx::MySqlPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub media: Arc<dyn MediaStore>,
    pub uia: Arc<UiaStore>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(pool: MySqlPool, media: Arc<dyn MediaStore>) -> Self {
        Self {
            pool,
            media,
            uia: UiaStore::new(),
            http: reqwest::Client::new(),
        }
    }
}
