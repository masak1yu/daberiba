use crate::media_store::MediaStore;
use sqlx::MySqlPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub media: Arc<dyn MediaStore>,
}

impl AppState {
    pub fn new(pool: MySqlPool, media: Arc<dyn MediaStore>) -> Self {
        Self { pool, media }
    }
}
