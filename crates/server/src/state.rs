use crate::media_store::MediaStore;
use crate::signing_key::ServerSigningKey;
use crate::sso::SsoConfig;
use crate::typing_store::TypingStore;
use crate::uia::UiaStore;
use dashmap::DashMap;
use sqlx::MySqlPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub media: Arc<dyn MediaStore>,
    pub uia: Arc<UiaStore>,
    pub typing: Arc<TypingStore>,
    pub http: reqwest::Client,
    /// 起動時に env から読んでキャッシュした SERVER_NAME
    pub server_name: Arc<str>,
    /// サーバー署名鍵（Federation 用）
    pub signing_key: Arc<ServerSigningKey>,
    /// Federation 公開鍵キャッシュ: "{server_name}/{key_id}" -> (key_bytes, valid_until_ms)
    pub fed_key_cache: Arc<DashMap<String, (Vec<u8>, u64)>>,
    /// SSO/OIDC 設定（OIDC_ISSUER が未設定の場合は None）
    pub sso: Option<Arc<SsoConfig>>,
}

impl AppState {
    pub async fn new(pool: MySqlPool, media: Arc<dyn MediaStore>) -> Self {
        let signing_key = Arc::new(ServerSigningKey::load_or_generate(&pool).await);
        let server_name: Arc<str> = std::env::var("SERVER_NAME")
            .unwrap_or_else(|_| "localhost".to_string())
            .into();
        let http = reqwest::Client::new();
        let sso = SsoConfig::load_from_env(&http).await.map(Arc::new);
        Self {
            pool,
            media,
            uia: UiaStore::new(),
            typing: TypingStore::new(),
            http,
            server_name,
            signing_key,
            fed_key_cache: Arc::new(DashMap::new()),
            sso,
        }
    }
}
