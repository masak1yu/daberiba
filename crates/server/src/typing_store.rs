use dashmap::DashMap;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// ルームごとのタイピング状態エントリ
struct RoomTyping {
    /// user_id -> expires_at
    users: HashMap<String, Instant>,
    /// このルームの typing が最後に変化したバージョン番号
    version: u64,
}

/// ルームごとのタイピング状態をインメモリ管理。
/// バージョン番号で差分配信をサポートする。
pub struct TypingStore {
    inner: DashMap<String, RoomTyping>,
    /// グローバルバージョンカウンター（typing 変化のたびにインクリメント）
    global_version: AtomicU64,
}

impl Default for TypingStore {
    fn default() -> Self {
        Self {
            inner: DashMap::new(),
            global_version: AtomicU64::new(0),
        }
    }
}

impl TypingStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// タイピング開始（timeout_ms ミリ秒後に自動失効）
    pub fn set(&self, room_id: &str, user_id: &str, timeout_ms: u64) {
        let expires = Instant::now() + Duration::from_millis(timeout_ms);
        let ver = self.global_version.fetch_add(1, Ordering::Relaxed) + 1;
        let mut entry = self
            .inner
            .entry(room_id.to_owned())
            .or_insert_with(|| RoomTyping {
                users: HashMap::new(),
                version: 0,
            });
        entry.users.insert(user_id.to_owned(), expires);
        entry.version = ver;
    }

    /// タイピング停止
    pub fn unset(&self, room_id: &str, user_id: &str) {
        if let Some(mut room) = self.inner.get_mut(room_id) {
            if room.users.remove(user_id).is_some() {
                let ver = self.global_version.fetch_add(1, Ordering::Relaxed) + 1;
                room.version = ver;
            }
        }
    }

    /// 現在タイピング中のユーザー一覧（期限切れを除外）
    pub fn get_typing(&self, room_id: &str) -> Vec<String> {
        let now = Instant::now();
        self.inner
            .get(room_id)
            .map(|room| {
                room.users
                    .iter()
                    .filter(|(_, &expires)| expires > now)
                    .map(|(uid, _)| uid.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// since_version より後に変化したルームの typing 情報を返す。
    /// 戻り値: (room_id, typing_users) のリストと現在の最大バージョン番号。
    pub fn get_changed_since(&self, since_version: u64) -> (Vec<(String, Vec<String>)>, u64) {
        let now = Instant::now();
        let current = self.global_version.load(Ordering::Relaxed);
        let changed = self
            .inner
            .iter()
            .filter(|entry| entry.version > since_version)
            .map(|entry| {
                let room_id = entry.key().clone();
                let users: Vec<String> = entry
                    .users
                    .iter()
                    .filter(|(_, &exp)| exp > now)
                    .map(|(uid, _)| uid.clone())
                    .collect();
                (room_id, users)
            })
            .collect();
        (changed, current)
    }

    /// 現在のグローバルバージョン番号を返す（next_batch 埋め込み用）
    #[allow(dead_code)]
    pub fn current_version(&self) -> u64 {
        self.global_version.load(Ordering::Relaxed)
    }
}
