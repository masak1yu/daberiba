/// OIDC プロバイダー設定。
///
/// 環境変数から読み込む。OIDC_ISSUER が設定されていれば discovery エンドポイント
/// (`{issuer}/.well-known/openid-configuration`) から各 URL を自動取得する。
/// OIDC_ISSUER が未設定の場合は SSO を無効とする。
///
/// 必須環境変数（OIDC_ISSUER が設定されている場合）:
/// - `OIDC_CLIENT_ID`      — クライアント ID
/// - `OIDC_CLIENT_SECRET`  — クライアントシークレット
/// - `OIDC_REDIRECT_URI`   — コールバック URL（例: https://your-server/_matrix/client/v3/login/sso/callback）
///
/// オプション（discovery で自動取得されない場合に個別指定）:
/// - `OIDC_AUTH_URL`        — 認可エンドポイント
/// - `OIDC_TOKEN_URL`       — トークンエンドポイント
/// - `OIDC_USERINFO_URL`    — ユーザー情報エンドポイント
#[derive(Clone, Debug)]
pub struct SsoConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    /// 表示用プロバイダー名（`/login` レスポンスの `identity_providers` に使用）
    pub provider_name: String,
}

impl SsoConfig {
    /// 環境変数から SSO 設定を読み込む。OIDC_ISSUER 未設定の場合は None。
    pub async fn load_from_env(http: &reqwest::Client) -> Option<Self> {
        let issuer = std::env::var("OIDC_ISSUER").ok()?;
        let client_id = std::env::var("OIDC_CLIENT_ID")
            .expect("OIDC_CLIENT_ID must be set when OIDC_ISSUER is configured");
        let client_secret = std::env::var("OIDC_CLIENT_SECRET")
            .expect("OIDC_CLIENT_SECRET must be set when OIDC_ISSUER is configured");
        let redirect_uri = std::env::var("OIDC_REDIRECT_URI")
            .expect("OIDC_REDIRECT_URI must be set when OIDC_ISSUER is configured");
        let provider_name =
            std::env::var("OIDC_PROVIDER_NAME").unwrap_or_else(|_| "SSO".to_string());

        // OIDC discovery ドキュメントを取得して各 URL を解決する
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let doc: serde_json::Value = match http.get(&discovery_url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("OIDC discovery parse failed: {}", e);
                    return None;
                }
            },
            Err(e) => {
                tracing::warn!("OIDC discovery fetch failed ({}): {}", discovery_url, e);
                return None;
            }
        };

        let auth_url = std::env::var("OIDC_AUTH_URL").ok().unwrap_or_else(|| {
            doc["authorization_endpoint"]
                .as_str()
                .unwrap_or("")
                .to_string()
        });
        let token_url = std::env::var("OIDC_TOKEN_URL")
            .ok()
            .unwrap_or_else(|| doc["token_endpoint"].as_str().unwrap_or("").to_string());
        let userinfo_url = std::env::var("OIDC_USERINFO_URL")
            .ok()
            .unwrap_or_else(|| doc["userinfo_endpoint"].as_str().unwrap_or("").to_string());

        if auth_url.is_empty() || token_url.is_empty() || userinfo_url.is_empty() {
            tracing::warn!("OIDC discovery did not return required endpoints; SSO disabled");
            return None;
        }

        tracing::info!(
            issuer = %issuer,
            provider = %provider_name,
            "SSO/OIDC enabled"
        );

        Some(Self {
            client_id,
            client_secret,
            redirect_uri,
            auth_url,
            token_url,
            userinfo_url,
            provider_name,
        })
    }
}
