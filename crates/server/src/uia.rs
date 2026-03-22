/// User Interactive Authentication (UIA) helpers
///
/// Matrix spec: https://spec.matrix.org/v1.x/client-server-api/#user-interactive-authentication-api
/// 本実装では m.login.password ステージのみサポートする。
/// セッションは DashMap でインメモリ管理し、5 分 TTL で失効する。
use axum::{http::StatusCode, Json};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const SESSION_TTL: Duration = Duration::from_secs(5 * 60);

/// インメモリ UIA セッションストア。
/// session_id → 作成時刻 を管理する。
pub struct UiaStore(DashMap<String, Instant>);

impl UiaStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(DashMap::new()))
    }

    /// 新しいセッション ID を発行してストアに保存し返す。
    fn create_session(&self) -> String {
        let id = Uuid::new_v4().to_string().replace('-', "");
        self.0.insert(id.clone(), Instant::now());
        id
    }

    /// セッション ID を検証する。
    /// 存在かつ TTL 内であれば true を返し、ストアから削除する（一回限り有効）。
    /// 期限切れ・存在しない場合は false。
    pub fn validate(&self, session_id: &str) -> bool {
        if let Some((_, created_at)) = self.0.remove(session_id) {
            return created_at.elapsed() < SESSION_TTL;
        }
        false
    }

    /// UIA チャレンジレスポンス（401）を返す。
    /// 新しいセッションを発行してレスポンスに含める。
    pub fn challenge(&self) -> (StatusCode, Json<serde_json::Value>) {
        let session = self.create_session();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_session_passes() {
        let store = UiaStore::new();
        let session = store.create_session();
        assert!(store.validate(&session));
    }

    #[test]
    fn unknown_session_fails() {
        let store = UiaStore::new();
        assert!(!store.validate("nonexistent"));
    }

    #[test]
    fn session_is_one_time_use() {
        let store = UiaStore::new();
        let session = store.create_session();
        assert!(store.validate(&session));
        assert!(!store.validate(&session)); // 2回目は失敗
    }

    #[test]
    fn challenge_embeds_session_id() {
        let store = UiaStore::new();
        let (status, Json(body)) = store.challenge();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let session_id = body["session"].as_str().unwrap();
        // 発行されたセッションが存在する（まだ validate していない）
        assert!(store.0.contains_key(session_id));
    }
}
