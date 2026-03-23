use crate::media_store::MediaStore;
use crate::signing_key::ServerSigningKey;
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
    /// サーバー署名鍵（Federation 用）
    pub signing_key: Arc<ServerSigningKey>,
}

impl AppState {
    pub fn new(pool: MySqlPool, media: Arc<dyn MediaStore>) -> Self {
        Self {
            pool,
            media,
            uia: UiaStore::new(),
            typing: TypingStore::new(),
            http: reqwest::Client::new(),
            signing_key: Arc::new(ServerSigningKey::generate()),
        }
    }
}
