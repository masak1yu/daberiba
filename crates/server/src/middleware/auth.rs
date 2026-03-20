use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use crate::{error::AppError, state::AppState};

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

    req.extensions_mut().insert(AuthUser { user_id, device_id, token });
    Ok(next.run(req).await)
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
