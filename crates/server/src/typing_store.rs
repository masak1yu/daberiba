use dashmap::DashMap;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// ルームごとのタイピング状態をインメモリ管理
/// room_id -> { user_id -> expires_at }
#[derive(Default)]
pub struct TypingStore {
    inner: DashMap<String, HashMap<String, Instant>>,
}

impl TypingStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// タイピング開始（timeout_ms ミリ秒後に自動失効）
    pub fn set(&self, room_id: &str, user_id: &str, timeout_ms: u64) {
        let expires = Instant::now() + Duration::from_millis(timeout_ms);
        let mut entry = self.inner.entry(room_id.to_owned()).or_default();
        entry.insert(user_id.to_owned(), expires);
    }

    /// タイピング停止
    pub fn unset(&self, room_id: &str, user_id: &str) {
        if let Some(mut users) = self.inner.get_mut(room_id) {
            users.remove(user_id);
        }
    }

    /// 現在タイピング中のユーザー一覧（期限切れを除外）
    pub fn get_typing(&self, room_id: &str) -> Vec<String> {
        let now = Instant::now();
        self.inner
            .get(room_id)
            .map(|users| {
                users
                    .iter()
                    .filter(|(_, &expires)| expires > now)
                    .map(|(uid, _)| uid.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}
