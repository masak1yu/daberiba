use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/register", post(register))
        .route(
            "/_matrix/client/v3/register/available",
            get(register_available),
        )
        .route("/_matrix/client/v3/login", get(login_flows).post(login))
        // SSO リダイレクト（認証不要 — ブラウザからアクセスされる）
        .route("/_matrix/client/v3/login/sso/redirect", get(sso_redirect))
        .route(
            "/_matrix/client/v3/login/sso/redirect/:idpId",
            get(sso_redirect_idp),
        )
        // SSO コールバック（認証不要 — OIDC プロバイダーからのリダイレクト）
        .route("/_matrix/client/v3/login/sso/callback", get(sso_callback))
}

/// 認証必須ルート（router.rs 側で auth middleware を付けて登録する）
pub fn protected_routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v1/login/get_token", post(get_login_token))
}

async fn login_flows(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut flows = vec![
        serde_json::json!({ "type": "m.login.password" }),
        serde_json::json!({ "type": "m.login.token" }),
    ];

    // SSO が有効な場合は m.login.sso を追加
    if let Some(sso) = &state.sso {
        flows.push(serde_json::json!({ "type": "m.login.sso" }));
        return Json(serde_json::json!({
            "flows": flows,
            "identity_providers": [{
                "id": "oidc",
                "name": sso.provider_name,
            }]
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
    // Matrix localpart: 英数字・アンダースコア・ハイフン・ドットのみ
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
        // 重複ユーザー
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

            // トークンを消費して user_id を取得
            let user_id = db::login_tokens::consume(&state.pool, &token)
                .await
                .map_err(AppError::Internal)?
                .ok_or(AppError::Unauthorized)?;

            // 新しいデバイスとアクセストークンを発行
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

/// POST /_matrix/client/v1/login/get_token
/// 現在のセッションから短命ログイントークンを発行する（クロスデバインログイン用）。
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

/// GET /_matrix/client/v3/register/available?username=<localpart>
/// ユーザー名が利用可能か確認する。利用可能なら { available: true }。
async fn register_available(
    State(state): State<AppState>,
    Query(params): Query<RegisterAvailableQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let localpart = params
        .username
        .ok_or_else(|| AppError::BadRequest("username required".into()))?;

    // ローカルパートを user_id 形式に変換
    let server_name = std::env::var("SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
    let user_id = format!("@{}:{}", localpart, server_name);

    let exists = db::users::exists(&state.pool, &user_id).await?;
    if exists {
        return Err(AppError::BadRequest("M_USER_IN_USE".into()));
    }

    Ok(Json(serde_json::json!({ "available": true })))
}

// ──────────────────────────────────────────────────────────────────────────────
// SSO / OIDC フロー
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SsoRedirectQuery {
    #[serde(rename = "redirectUrl")]
    redirect_url: String,
}

/// GET /_matrix/client/v3/login/sso/redirect?redirectUrl=<url>
/// OIDC 認可エンドポイントへリダイレクトする。SSO が無効な場合は 400。
async fn sso_redirect(
    State(state): State<AppState>,
    Query(params): Query<SsoRedirectQuery>,
) -> Result<Redirect, AppError> {
    sso_redirect_impl(&state, &params.redirect_url).await
}

/// GET /_matrix/client/v3/login/sso/redirect/:idpId?redirectUrl=<url>
/// 特定プロバイダー指定（現状はプロバイダーが 1 つのみなので id は無視）。
async fn sso_redirect_idp(
    State(state): State<AppState>,
    Path(_idp_id): Path<String>,
    Query(params): Query<SsoRedirectQuery>,
) -> Result<Redirect, AppError> {
    sso_redirect_impl(&state, &params.redirect_url).await
}

async fn sso_redirect_impl(state: &AppState, redirect_url: &str) -> Result<Redirect, AppError> {
    let sso = state
        .sso
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("SSO is not configured on this server".into()))?;

    // state トークンを生成して DB に保存
    let state_token = db::sso::create_state(&state.pool, redirect_url)
        .await
        .map_err(AppError::Internal)?;

    // OIDC 認可 URL を組み立てる
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
        sso.auth_url,
        urlencoding::encode(&sso.client_id),
        urlencoding::encode(&sso.redirect_uri),
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
/// OIDC プロバイダーからのコールバック。
/// code を token に交換し、userinfo を取得してログイントークンを発行する。
async fn sso_callback(
    State(state): State<AppState>,
    Query(params): Query<SsoCallbackQuery>,
) -> Result<Redirect, AppError> {
    // エラー応答チェック
    if let Some(err) = params.error {
        return Err(AppError::BadRequest(format!("SSO error: {}", err)));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::BadRequest("missing code".into()))?;
    let state_token = params
        .state
        .ok_or_else(|| AppError::BadRequest("missing state".into()))?;

    let sso = state
        .sso
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("SSO is not configured".into()))?;

    // state を消費して redirect_url を取得
    let redirect_url = db::sso::consume_state(&state.pool, &state_token)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::BadRequest("invalid or expired state".into()))?;

    // Authorization code を token に交換
    let token_resp = state
        .http
        .post(&sso.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &sso.redirect_uri),
            ("client_id", &sso.client_id),
            ("client_secret", &sso.client_secret),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let token_json: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let access_token = token_json["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no access_token in OIDC response")))?;

    // userinfo エンドポイントからユーザー情報を取得
    let userinfo: serde_json::Value = state
        .http
        .get(&sso.userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let sub = userinfo["sub"]
        .as_str()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no sub in OIDC userinfo")))?;

    // プロバイダー固有の composite sub キー（将来的な複数プロバイダー対応）
    let composite_sub = format!("oidc:{}", sub);

    // 既存マッピングを探すか、新規ユーザーを作成する
    let matrix_user_id = if let Some(uid) = db::sso::find_user_by_sub(&state.pool, &composite_sub)
        .await
        .map_err(AppError::Internal)?
    {
        uid
    } else {
        // ユーザー名を userinfo から決定する
        let localpart = derive_localpart(&userinfo);
        let unique_localpart =
            make_unique_localpart(&state.pool, &localpart, &state.server_name).await?;

        // ランダムパスワードで登録（SSO ユーザーはパスワードログイン不可）
        let random_pw = uuid::Uuid::new_v4().to_string();
        let (uid, _tok, _dev) = db::users::register(
            &state.pool,
            &unique_localpart,
            &random_pw,
            &state.server_name,
        )
        .await
        .map_err(AppError::Internal)?;

        // マッピングを保存
        db::sso::link_account(&state.pool, &composite_sub, &uid)
            .await
            .map_err(AppError::Internal)?;

        // display_name を userinfo から設定（ベストエフォート）
        let display_name = userinfo["name"]
            .as_str()
            .or_else(|| userinfo["preferred_username"].as_str());
        if let Some(dn) = display_name {
            let _ = db::profile::set_displayname(&state.pool, &uid, Some(dn)).await;
        }

        uid
    };

    // Matrix ログイントークンを発行して client にリダイレクト
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

/// userinfo クレームから Matrix ローカルパートを導出する。
/// preferred_username → email ローカルパート → sub の先頭 16 文字 の順に試みる。
fn derive_localpart(userinfo: &serde_json::Value) -> String {
    let raw = userinfo["preferred_username"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            userinfo["email"]
                .as_str()
                .and_then(|e| e.split('@').next())
                .map(str::to_string)
        })
        .or_else(|| {
            userinfo["sub"]
                .as_str()
                .map(|s| s.chars().take(16).collect())
        })
        .unwrap_or_else(|| "sso_user".to_string());

    // Matrix localpart 制約: a-z0-9_-. のみ、小文字化
    sanitize_localpart(&raw)
}

/// 文字列を Matrix localpart 制約に合わせてサニタイズする。
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

/// ローカルパートが重複する場合に数字サフィックスを付けてユニークにする。
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
