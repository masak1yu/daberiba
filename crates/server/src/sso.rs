use anyhow::Result;

#[derive(Clone, Debug, PartialEq)]
pub enum ProviderKind {
    Oidc,
    Github,
}

/// SSO プロバイダー設定。
#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    /// None = id_token から sub を抽出（Apple など userinfo エンドポイントがない場合）
    pub userinfo_url: Option<String>,
    pub scopes: String,
    pub kind: ProviderKind,
}

/// 環境変数から有効なプロバイダーをすべてロードする。
///
/// 各プロバイダーは対応する CLIENT_ID が設定されていれば有効。
/// すべてのプロバイダーで `SSO_REDIRECT_URI` が必要。
pub async fn load_providers(http: &reqwest::Client) -> Vec<ProviderConfig> {
    let mut providers = Vec::new();
    if let Some(p) = load_google(http).await {
        providers.push(p);
    }
    if let Some(p) = load_github() {
        providers.push(p);
    }
    if let Some(p) = load_apple(http).await {
        providers.push(p);
    }
    providers
}

fn sso_redirect_uri() -> Option<String> {
    std::env::var("SSO_REDIRECT_URI").ok()
}

async fn load_google(http: &reqwest::Client) -> Option<ProviderConfig> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID").ok()?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").ok()?;
    let redirect_uri = sso_redirect_uri()?;
    let doc = fetch_oidc_discovery(http, "https://accounts.google.com").await?;
    let auth_url = doc["authorization_endpoint"].as_str()?.to_string();
    let token_url = doc["token_endpoint"].as_str()?.to_string();
    let userinfo_url = doc["userinfo_endpoint"].as_str().map(str::to_string);
    tracing::info!(provider = "google", "SSO provider enabled");
    Some(ProviderConfig {
        id: "google".into(),
        name: "Google".into(),
        client_id,
        client_secret,
        redirect_uri,
        auth_url,
        token_url,
        userinfo_url,
        scopes: "openid email profile".into(),
        kind: ProviderKind::Oidc,
    })
}

fn load_github() -> Option<ProviderConfig> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").ok()?;
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET").ok()?;
    let redirect_uri = sso_redirect_uri()?;
    tracing::info!(provider = "github", "SSO provider enabled");
    Some(ProviderConfig {
        id: "github".into(),
        name: "GitHub".into(),
        client_id,
        client_secret,
        redirect_uri,
        auth_url: "https://github.com/login/oauth/authorize".into(),
        token_url: "https://github.com/login/oauth/access_token".into(),
        userinfo_url: Some("https://api.github.com/user".into()),
        scopes: "read:user user:email".into(),
        kind: ProviderKind::Github,
    })
}

async fn load_apple(http: &reqwest::Client) -> Option<ProviderConfig> {
    let client_id = std::env::var("APPLE_CLIENT_ID").ok()?;
    let team_id = std::env::var("APPLE_TEAM_ID").ok()?;
    let key_id = std::env::var("APPLE_KEY_ID").ok()?;
    let private_key = std::env::var("APPLE_PRIVATE_KEY").ok()?;
    let redirect_uri = sso_redirect_uri()?;
    let client_secret =
        match generate_apple_client_secret(&team_id, &key_id, &client_id, &private_key) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Apple client secret generation failed: {}", e);
                return None;
            }
        };
    let doc = fetch_oidc_discovery(http, "https://appleid.apple.com").await?;
    let auth_url = doc["authorization_endpoint"].as_str()?.to_string();
    let token_url = doc["token_endpoint"].as_str()?.to_string();
    tracing::info!(provider = "apple", "SSO provider enabled");
    Some(ProviderConfig {
        id: "apple".into(),
        name: "Apple".into(),
        client_id,
        client_secret,
        redirect_uri,
        auth_url,
        token_url,
        // Apple の userinfo_endpoint は sub のみ返す; id_token から取得する方が確実
        userinfo_url: None,
        scopes: "name email".into(),
        kind: ProviderKind::Oidc,
    })
}

async fn fetch_oidc_discovery(http: &reqwest::Client, issuer: &str) -> Option<serde_json::Value> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => r.json().await.ok(),
        Ok(r) => {
            tracing::warn!("OIDC discovery returned {} for {}", r.status(), issuer);
            None
        }
        Err(e) => {
            tracing::warn!("OIDC discovery failed for {}: {}", issuer, e);
            None
        }
    }
}

/// Apple 用 ES256 JWT クライアントシークレット（有効期限 6 ヶ月）。
/// `private_key_pem` は PKCS#8 EC 秘密鍵（.p8 ファイルの内容）。
pub fn generate_apple_client_secret(
    team_id: &str,
    key_id: &str,
    client_id: &str,
    private_key_pem: &str,
) -> Result<String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": team_id,
        "iat": now,
        "exp": now + 15_777_000i64,
        "aud": "https://appleid.apple.com",
        "sub": client_id,
    });
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_string());
    let key = EncodingKey::from_ec_pem(private_key_pem.as_bytes())?;
    Ok(encode(&header, &claims, &key)?)
}
