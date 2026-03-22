/// User Interactive Authentication (UIA) helpers
///
/// Matrix spec: https://spec.matrix.org/v1.x/client-server-api/#user-interactive-authentication-api
/// 本実装では m.login.password ステージのみサポートする。
use axum::{http::StatusCode, response::IntoResponse, Json};
use uuid::Uuid;

/// UIA チャレンジレスポンス（401）を生成する。
/// クライアントはこれを受け取ったら auth フィールド付きで再リクエストする。
pub fn challenge() -> impl IntoResponse {
    let session = Uuid::new_v4().to_string().replace('-', "");
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "session": session,
            "flows": [{ "stages": ["m.login.password"] }],
            "params": {},
            "errcode": "M_UNAUTHORIZED",
            "error": "Additional authentication required",
        })),
    )
}

/// `auth` フィールドから m.login.password のパスワードを取り出す。
/// `auth.type` が "m.login.password" でなければ None を返す。
pub fn extract_password(auth: &serde_json::Value) -> Option<&str> {
    if auth.get("type")?.as_str()? == "m.login.password" {
        auth.get("password")?.as_str()
    } else {
        None
    }
}
