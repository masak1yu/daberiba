use std::collections::HashSet;

/// Matrix フィルター定義（POST /filter で保存された JSON から構築）
pub struct FilterDef {
    // room フィルター
    pub rooms: Option<HashSet<String>>,
    pub not_rooms: Option<HashSet<String>>,
    pub timeline_types: Option<HashSet<String>>,
    pub timeline_not_types: Option<HashSet<String>>,
    pub timeline_limit: Option<u32>,
    pub state_types: Option<HashSet<String>>,
    pub state_not_types: Option<HashSet<String>>,
    #[allow(dead_code)]
    pub state_limit: Option<u32>, // 将来の初回 sync ステート制限用
    pub ephemeral_types: Option<HashSet<String>>,
    pub ephemeral_not_types: Option<HashSet<String>>,
    pub account_data_types: Option<HashSet<String>>,
    pub account_data_not_types: Option<HashSet<String>>,
    // トップレベル
    pub presence_types: Option<HashSet<String>>,
    pub presence_not_types: Option<HashSet<String>>,
}

impl FilterDef {
    pub fn from_json(v: &serde_json::Value) -> Self {
        let room = v.get("room");
        let timeline = room.and_then(|r| r.get("timeline"));
        let state = room.and_then(|r| r.get("state"));
        let ephemeral = room.and_then(|r| r.get("ephemeral"));
        let account_data = room.and_then(|r| r.get("account_data"));
        let presence = v.get("presence");

        FilterDef {
            rooms: extract_set(room, "rooms"),
            not_rooms: extract_set(room, "not_rooms"),
            timeline_types: extract_set(timeline, "types"),
            timeline_not_types: extract_set(timeline, "not_types"),
            timeline_limit: extract_u32(timeline, "limit"),
            state_types: extract_set(state, "types"),
            state_not_types: extract_set(state, "not_types"),
            state_limit: extract_u32(state, "limit"),
            ephemeral_types: extract_set(ephemeral, "types"),
            ephemeral_not_types: extract_set(ephemeral, "not_types"),
            account_data_types: extract_set(account_data, "types"),
            account_data_not_types: extract_set(account_data, "not_types"),
            presence_types: extract_set(presence, "types"),
            presence_not_types: extract_set(presence, "not_types"),
        }
    }

    /// ルームをフィルターに含めるか
    pub fn include_room(&self, room_id: &str) -> bool {
        if let Some(ref rooms) = self.rooms {
            if !rooms.contains(room_id) {
                return false;
            }
        }
        if let Some(ref not_rooms) = self.not_rooms {
            if not_rooms.contains(room_id) {
                return false;
            }
        }
        true
    }

    /// イベント配列にタイプフィルターを適用
    pub fn apply_event_filter(
        events: &mut Vec<serde_json::Value>,
        types: &Option<HashSet<String>>,
        not_types: &Option<HashSet<String>>,
    ) {
        events.retain(|e| {
            let t = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(ref allowed) = types {
                if !allowed.contains(t) {
                    return false;
                }
            }
            if let Some(ref denied) = not_types {
                if denied.contains(t) {
                    return false;
                }
            }
            true
        });
    }
}

fn extract_set(obj: Option<&serde_json::Value>, key: &str) -> Option<HashSet<String>> {
    obj?.get(key)?.as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
}

fn extract_u32(obj: Option<&serde_json::Value>, key: &str) -> Option<u32> {
    obj?.get(key)?
        .as_u64()
        .map(|v| v.min(u32::MAX as u64) as u32)
}
