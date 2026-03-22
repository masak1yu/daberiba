use crate::{error::AppError, state::AppState};
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;

/// Matrix access token を Bearer または query param から抽出して検証する
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_access_token(&req)?;

    let (user_id, device_id) = db::access_tokens::verify(&state.pool, &token)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // last_seen を非同期で更新（失敗しても認証は通す）
    let now_ms = chrono::Utc::now().timestamp_millis();
    let ip = extract_client_ip(&req);
    let pool = state.pool.clone();
    let uid = user_id.clone();
    let did = device_id.clone();
    tokio::spawn(async move {
        if let Err(e) =
            db::devices::update_last_seen(&pool, &uid, &did, now_ms, ip.as_deref()).await
        {
            tracing::warn!("failed to update last_seen: {e}");
        }
    });

    req.extensions_mut().insert(AuthUser {
        user_id,
        device_id,
        token,
    });
    Ok(next.run(req).await)
}

/// クライアント IP を取得する。
/// X-Real-IP → X-Forwarded-For の先頭 → ConnectInfo の順で試みる。
fn extract_client_ip(req: &Request) -> Option<String> {
    if let Some(v) = req.headers().get("X-Real-IP") {
        if let Ok(s) = v.to_str() {
            return Some(s.to_string());
        }
    }
    if let Some(v) = req.headers().get("X-Forwarded-For") {
        if let Ok(s) = v.to_str() {
            let first = s.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return Some(first.to_string());
            }
        }
    }
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
}

fn extract_access_token(req: &Request) -> Result<String, AppError> {
    // Bearer token
    if let Some(auth) = req.headers().get("Authorization") {
        let val = auth.to_str().map_err(|_| AppError::Unauthorized)?;
        if let Some(token) = val.strip_prefix("Bearer ") {
            return Ok(token.to_string());
        }
    }

    // query param: ?access_token=...
    if let Some(query) = req.uri().query() {
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            if k == "access_token" {
                return Ok(v.to_string());
            }
        }
    }

    Err(AppError::Unauthorized)
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub device_id: String,
    pub token: String,
}
