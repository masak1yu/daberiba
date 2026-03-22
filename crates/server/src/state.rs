use crate::media_store::MediaStore;
use crate::typing_store::TypingStore;
use crate::uia::UiaStore;
use sqlx::MySqlPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub media: Arc<dyn MediaStore>,
    pub uia: Arc<UiaStore>,
    pub typing: Arc<TypingStore>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(pool: MySqlPool, media: Arc<dyn MediaStore>) -> Self {
        Self {
            pool,
            media,
            uia: UiaStore::new(),
            typing: TypingStore::new(),
            http: reqwest::Client::new(),
        }
    }
}
