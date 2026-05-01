use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    sso::{ProviderConfig, ProviderKind},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/register", post(register))
        .route(
            "/_matrix/client/v3/register/available",
            get(register_available),
        )
        .route("/_matrix/client/v3/login", get(login_flows).post(login))
        .route("/_matrix/client/v3/login/sso/redirect", get(sso_redirect))
        .route(
            "/_matrix/client/v3/login/sso/redirect/:idpId",
            get(sso_redirect_idp),
        )
        .route("/_matrix/client/v3/login/sso/callback", get(sso_callback))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v1/login/get_token", post(get_login_token))
}

async fn login_flows(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut flows = vec![
        serde_json::json!({ "type": "m.login.password" }),
        serde_json::json!({ "type": "m.login.token" }),
    ];

    if !state.sso_providers.is_empty() {
        flows.push(serde_json::json!({ "type": "m.login.sso" }));
        let identity_providers: Vec<_> = state
            .sso_providers
            .iter()
            .map(|p| serde_json::json!({ "id": p.id, "name": p.name }))
            .collect();
        return Json(serde_json::json!({
            "flows": flows,
            "identity_providers": identity_providers,
        }));
    }

    Json(serde_json::json!({ "flows": flows }))
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    user_id: String,
    access_token: String,
    device_id: String,
}

fn validate_username(username: &str) -> Result<(), AppError> {
    if username.is_empty() {
        return Err(AppError::BadRequest("username must not be empty".into()));
    }
    if username.len() > 255 {
        return Err(AppError::BadRequest("username too long".into()));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(AppError::BadRequest(
            "username may only contain a-z, 0-9, _, -, .".into(),
        ));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Json<RegisterResponse>> {
    validate_username(&body.username)?;
    validate_password(&body.password)?;

    let (user_id, access_token, device_id) = db::users::register(
        &state.pool,
        &body.username,
        &body.password,
        &state.server_name,
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("Duplicate") || e.to_string().contains("duplicate") {
            AppError::BadRequest("username already taken".into())
        } else {
            AppError::Internal(e)
        }
    })?;

    Ok(Json(RegisterResponse {
        user_id,
        access_token,
        device_id,
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    #[serde(rename = "type")]
    login_type: String,
    identifier: Option<LoginIdentifier>,
    password: Option<String>,
    token: Option<String>,
}

#[derive(Deserialize)]
struct LoginIdentifier {
    #[serde(rename = "type")]
    _id_type: String,
    user: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    user_id: String,
    access_token: String,
    device_id: String,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    match body.login_type.as_str() {
        "m.login.password" => {
            let username = body
                .identifier
                .and_then(|id| id.user)
                .ok_or_else(|| AppError::BadRequest("missing identifier.user".into()))?;
            let password = body
                .password
                .ok_or_else(|| AppError::BadRequest("missing password".into()))?;
            let (user_id, access_token, device_id) =
                db::users::login(&state.pool, &username, &password, &state.server_name)
                    .await
                    .map_err(|_| AppError::Unauthorized)?;
            Ok(Json(LoginResponse {
                user_id,
                access_token,
                device_id,
            }))
        }
        "m.login.token" => {
            let token = body
                .token
                .ok_or_else(|| AppError::BadRequest("missing token".into()))?;
            let user_id = db::login_tokens::consume(&state.pool, &token)
                .await
                .map_err(AppError::Internal)?
                .ok_or(AppError::Unauthorized)?;
            let device_id = uuid::Uuid::new_v4()
                .to_string()
                .replace('-', "")
                .to_uppercase();
            let device_id = format!("DEVICE_{}", &device_id[..8]);
            db::devices::create(&state.pool, &user_id, &device_id)
                .await
                .map_err(AppError::Internal)?;
            let access_token = db::access_tokens::create(&state.pool, &user_id, &device_id)
                .await
                .map_err(AppError::Internal)?;
            Ok(Json(LoginResponse {
                user_id,
                access_token,
                device_id,
            }))
        }
        _ => Err(AppError::BadRequest("unsupported login type".into())),
    }
}

async fn get_login_token(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let token = db::login_tokens::create(&state.pool, &user.user_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(serde_json::json!({
        "login_token": token,
        "expires_in_ms": 120_000,
    })))
}

#[derive(Deserialize)]
struct RegisterAvailableQuery {
    username: Option<String>,
}

async fn register_available(
    State(state): State<AppState>,
    Query(params): Query<RegisterAvailableQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let localpart = params
        .username
        .ok_or_else(|| AppError::BadRequest("username required".into()))?;
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let user_id = format!("@{}:{}", localpart, server_name);
    let exists = db::users::exists(&state.pool, &user_id).await?;
    if exists {
        return Err(AppError::BadRequest("M_USER_IN_USE".into()));
    }
    Ok(Json(serde_json::json!({ "available": true })))
}

// ──────────────────────────────────────────────────────────────────────────────
// SSO / OAuth2 フロー
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SsoRedirectQuery {
    #[serde(rename = "redirectUrl")]
    redirect_url: String,
}

/// GET /_matrix/client/v3/login/sso/redirect?redirectUrl=<url>
/// 最初の有効プロバイダーへリダイレクト（単一プロバイダー後方互換）。
async fn sso_redirect(
    State(state): State<AppState>,
    Query(params): Query<SsoRedirectQuery>,
) -> Result<Redirect, AppError> {
    let provider = state
        .sso_providers
        .first()
        .ok_or_else(|| AppError::BadRequest("SSO is not configured on this server".into()))?;
    build_sso_redirect(&state, provider, &params.redirect_url).await
}

/// GET /_matrix/client/v3/login/sso/redirect/:idpId?redirectUrl=<url>
/// 指定プロバイダーへリダイレクト。
async fn sso_redirect_idp(
    State(state): State<AppState>,
    Path(idp_id): Path<String>,
    Query(params): Query<SsoRedirectQuery>,
) -> Result<Redirect, AppError> {
    let provider = state
        .sso_providers
        .iter()
        .find(|p| p.id == idp_id)
        .ok_or_else(|| AppError::BadRequest(format!("unknown SSO provider: {}", idp_id)))?;
    build_sso_redirect(&state, provider, &params.redirect_url).await
}

async fn build_sso_redirect(
    state: &AppState,
    provider: &ProviderConfig,
    redirect_url: &str,
) -> Result<Redirect, AppError> {
    let state_token = db::sso::create_state(&state.pool, redirect_url, &provider.id)
        .await
        .map_err(AppError::Internal)?;

    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
        provider.auth_url,
        urlencoding::encode(&provider.client_id),
        urlencoding::encode(&provider.redirect_uri),
        urlencoding::encode(&provider.scopes),
        state_token,
    );

    Ok(Redirect::temporary(&auth_url))
}

#[derive(Deserialize)]
struct SsoCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// GET /_matrix/client/v3/login/sso/callback?code=...&state=...
/// プロバイダーからのコールバック。code を token に交換し login_token を発行する。
async fn sso_callback(
    State(state): State<AppState>,
    Query(params): Query<SsoCallbackQuery>,
) -> Result<Redirect, AppError> {
    if let Some(err) = params.error {
        return Err(AppError::BadRequest(format!("SSO error: {}", err)));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::BadRequest("missing code".into()))?;
    let state_token = params
        .state
        .ok_or_else(|| AppError::BadRequest("missing state".into()))?;

    let (redirect_url, provider_id) = db::sso::consume_state(&state.pool, &state_token)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::BadRequest("invalid or expired state".into()))?;

    let provider = state
        .sso_providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("unknown provider: {}", provider_id)))?;

    let token_json = exchange_code(&state.http, provider, &code)
        .await
        .map_err(AppError::Internal)?;

    let user_info = extract_user_info(&state.http, provider, &token_json)
        .await
        .map_err(AppError::Internal)?;

    let composite_sub = format!("{}:{}", provider_id, user_info.sub);

    let matrix_user_id = if let Some(uid) = db::sso::find_user_by_sub(&state.pool, &composite_sub)
        .await
        .map_err(AppError::Internal)?
    {
        uid
    } else {
        let base = user_info
            .preferred_username
            .as_deref()
            .map(sanitize_localpart)
            .unwrap_or_else(|| sanitize_localpart(&user_info.sub));
        let localpart = make_unique_localpart(&state.pool, &base, &state.server_name).await?;
        let random_pw = uuid::Uuid::new_v4().to_string();
        let (uid, _, _) =
            db::users::register(&state.pool, &localpart, &random_pw, &state.server_name)
                .await
                .map_err(AppError::Internal)?;
        db::sso::link_account(&state.pool, &composite_sub, &uid)
            .await
            .map_err(AppError::Internal)?;
        if let Some(dn) = &user_info.display_name {
            let _ = db::profile::set_displayname(&state.pool, &uid, Some(dn)).await;
        }
        uid
    };

    let login_token = db::login_tokens::create(&state.pool, &matrix_user_id)
        .await
        .map_err(AppError::Internal)?;

    let final_url = if redirect_url.contains('?') {
        format!("{}&loginToken={}", redirect_url, login_token)
    } else {
        format!("{}?loginToken={}", redirect_url, login_token)
    };

    Ok(Redirect::temporary(&final_url))
}

// ──────────────────────────────────────────────────────────────────────────────
// OAuth2 トークン交換とユーザー情報取得
// ──────────────────────────────────────────────────────────────────────────────

struct ExtractedUser {
    sub: String,
    preferred_username: Option<String>,
    display_name: Option<String>,
}

async fn exchange_code(
    http: &reqwest::Client,
    provider: &ProviderConfig,
    code: &str,
) -> anyhow::Result<serde_json::Value> {
    let mut req = http.post(&provider.token_url).form(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &provider.redirect_uri),
        ("client_id", &provider.client_id),
        ("client_secret", &provider.client_secret),
    ]);
    // GitHub はデフォルトで form エンコードを返す; JSON が必要
    if provider.kind == ProviderKind::Github {
        req = req.header("Accept", "application/json");
    }
    let resp = req.send().await?;
    let json: serde_json::Value = resp.json().await?;
    if let Some(err) = json["error"].as_str() {
        anyhow::bail!("token exchange failed: {}", err);
    }
    Ok(json)
}

async fn extract_user_info(
    http: &reqwest::Client,
    provider: &ProviderConfig,
    token_json: &serde_json::Value,
) -> anyhow::Result<ExtractedUser> {
    match provider.kind {
        ProviderKind::Github => extract_github_user_info(http, token_json).await,
        ProviderKind::Oidc => extract_oidc_user_info(http, provider, token_json).await,
    }
}

async fn extract_github_user_info(
    http: &reqwest::Client,
    token_json: &serde_json::Value,
) -> anyhow::Result<ExtractedUser> {
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no access_token in GitHub response"))?;

    let userinfo: serde_json::Value = http
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "daberiba")
        .send()
        .await?
        .json()
        .await?;

    let id = userinfo["id"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("no id in GitHub userinfo"))?;
    let login = userinfo["login"].as_str().map(str::to_string);
    let display_name = userinfo["name"]
        .as_str()
        .or_else(|| userinfo["login"].as_str())
        .map(str::to_string);

    Ok(ExtractedUser {
        sub: id.to_string(),
        preferred_username: login,
        display_name,
    })
}

async fn extract_oidc_user_info(
    http: &reqwest::Client,
    provider: &ProviderConfig,
    token_json: &serde_json::Value,
) -> anyhow::Result<ExtractedUser> {
    if let Some(userinfo_url) = &provider.userinfo_url {
        // 標準 OIDC userinfo エンドポイント（Google など）
        let access_token = token_json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no access_token in OIDC response"))?;
        let userinfo: serde_json::Value = http
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await?
            .json()
            .await?;
        let sub = userinfo["sub"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no sub in OIDC userinfo"))?
            .to_string();
        let preferred_username = userinfo["preferred_username"]
            .as_str()
            .or_else(|| userinfo["email"].as_str().and_then(|e| e.split('@').next()))
            .map(str::to_string);
        let display_name = userinfo["name"]
            .as_str()
            .or_else(|| userinfo["preferred_username"].as_str())
            .map(str::to_string);
        Ok(ExtractedUser {
            sub,
            preferred_username,
            display_name,
        })
    } else {
        // id_token から sub を抽出（Apple — userinfo エンドポイントなし）
        let id_token = token_json["id_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no id_token in response"))?;
        let sub = extract_jwt_claim(id_token, "sub")
            .ok_or_else(|| anyhow::anyhow!("could not extract sub from id_token"))?;
        let email = extract_jwt_claim(id_token, "email");
        let preferred_username = email
            .as_deref()
            .and_then(|e| e.split('@').next())
            .map(str::to_string);
        Ok(ExtractedUser {
            sub,
            preferred_username,
            display_name: None,
        })
    }
}

/// JWT のペイロード（base64url）から指定クレームの文字列値を取り出す。署名検証はしない。
fn extract_jwt_claim(jwt: &str, claim: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims[claim].as_str().map(str::to_string)
}

// ──────────────────────────────────────────────────────────────────────────────
// ユーザー名ユーティリティ
// ──────────────────────────────────────────────────────────────────────────────

fn sanitize_localpart(s: &str) -> String {
    let out: String = s
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .take(64)
        .collect();
    if out.is_empty() {
        "sso_user".to_string()
    } else {
        out
    }
}

async fn make_unique_localpart(
    pool: &sqlx::MySqlPool,
    base: &str,
    server_name: &str,
) -> Result<String, AppError> {
    let user_id = format!("@{}:{}", base, server_name);
    if !db::users::exists(pool, &user_id)
        .await
        .map_err(AppError::Internal)?
    {
        return Ok(base.to_string());
    }
    for i in 2..=9999u32 {
        let candidate = format!("{}_{}", base, i);
        let uid = format!("@{}:{}", candidate, server_name);
        if !db::users::exists(pool, &uid)
            .await
            .map_err(AppError::Internal)?
        {
            return Ok(candidate);
        }
    }
    Err(AppError::Internal(anyhow::anyhow!(
        "could not find unique localpart for SSO user"
    )))
}
