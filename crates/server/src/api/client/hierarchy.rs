use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v1/rooms/:roomId/hierarchy",
        get(get_hierarchy),
    )
}

#[derive(Deserialize, Default)]
struct HierarchyQuery {
    /// ページネーション用 from トークン（room_id）
    from: Option<String>,
    limit: Option<u64>,
    /// true の場合、content.suggested=true の子のみ返す
    #[serde(default)]
    suggested_only: bool,
    /// 再帰探索の最大深さ（デフォルト 1、最大 5）
    max_depth: Option<u32>,
}

/// GET /_matrix/client/v1/rooms/:roomId/hierarchy
/// スペース階層を返す（MSC2946）。
/// max_depth まで再帰的に BFS 展開する。ページネーションはルート直下の子に適用。
async fn get_hierarchy(
    State(state): State<AppState>,
    axum::Extension(_user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(query): Query<HierarchyQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let limit = query.limit.unwrap_or(50).min(50) as usize;
    let max_depth = query.max_depth.unwrap_or(1).min(5) as usize;

    // ルート情報を収集する。
    let mut root = build_room_summary(&state.pool, &room_id).await?;

    // m.space.child 状態イベントを取得して子ルームを列挙する。
    let all_child_states = get_space_children(&state.pool, &room_id).await?;

    // suggested_only フィルタ
    let child_states: Vec<_> = if query.suggested_only {
        all_child_states
            .into_iter()
            .filter(|c| {
                c.get("content")
                    .and_then(|v| v.get("suggested"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .collect()
    } else {
        all_child_states
    };

    // from トークンによるオフセット処理（ルーム ID をカーソルとして使用）
    let start = if let Some(from) = &query.from {
        child_states
            .iter()
            .position(|c| {
                c.get("state_key")
                    .and_then(|v| v.as_str())
                    .map(|sk| sk == from)
                    .unwrap_or(false)
            })
            .map(|i| i + 1)
            .unwrap_or(0)
    } else {
        0
    };

    let page: Vec<_> = child_states.iter().skip(start).take(limit + 1).collect();
    let has_more = page.len() > limit;
    let page = &page[..page.len().min(limit)];

    root["children_state"] = serde_json::json!(child_states);

    // BFS でルート直下の子を展開し、max_depth まで再帰する。
    // visited は循環を防ぐ。
    let mut rooms = vec![root];
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(room_id.clone());

    // (room_id, current_depth)
    let mut queue: std::collections::VecDeque<(String, usize)> = page
        .iter()
        .filter_map(|c| {
            c.get("state_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .map(|rid| (rid, 1))
        .collect();

    while let Some((child_id, depth)) = queue.pop_front() {
        if !visited.insert(child_id.clone()) {
            continue;
        }

        let Ok(mut summary) = build_room_summary(&state.pool, &child_id).await else {
            continue;
        };

        if depth < max_depth {
            // 子の子を展開する（suggested_only は深い階層では適用しない）
            let grandchildren = get_space_children(&state.pool, &child_id)
                .await
                .unwrap_or_default();
            summary["children_state"] = serde_json::json!(grandchildren);
            for gc in &grandchildren {
                if let Some(gc_id) = gc.get("state_key").and_then(|v| v.as_str()) {
                    if !visited.contains(gc_id) {
                        queue.push_back((gc_id.to_string(), depth + 1));
                    }
                }
            }
        }

        rooms.push(summary);
    }

    let next_batch = if has_more {
        page.last()
            .and_then(|c| c.get("state_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "rooms": rooms,
        "next_batch": next_batch,
    })))
}

/// ルームの概要オブジェクトを構築する。
async fn build_room_summary(
    pool: &sqlx::MySqlPool,
    room_id: &str,
) -> anyhow::Result<serde_json::Value> {
    use sqlx::Row;

    // ルーム基本情報
    let row = sqlx::query("SELECT name, topic FROM rooms WHERE room_id = ?")
        .bind(room_id)
        .fetch_optional(pool)
        .await?;

    let (name, topic) = row
        .map(|r| {
            (
                r.get::<Option<String>, _>("name"),
                r.get::<Option<String>, _>("topic"),
            )
        })
        .unwrap_or((None, None));

    // 参加人数
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM room_memberships WHERE room_id = ? AND membership = 'join'",
    )
    .bind(room_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // join_rules
    let join_rule = db::room_state::get_event(pool, room_id, "m.room.join_rules", "")
        .await
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("join_rule")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "invite".to_string());

    // world_readable
    let world_readable = db::room_state::get_event(pool, room_id, "m.room.history_visibility", "")
        .await
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("history_visibility")
                .and_then(|h| h.as_str())
                .map(|s| s == "world_readable")
        })
        .unwrap_or(false);

    // guest_can_join
    let guest_can_join = db::room_state::get_event(pool, room_id, "m.room.guest_access", "")
        .await
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("guest_access")
                .and_then(|g| g.as_str())
                .map(|s| s == "can_join")
        })
        .unwrap_or(false);

    // room_type (スペースかどうか)
    let room_type = db::room_state::get_event(pool, room_id, "m.room.create", "")
        .await
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        });

    // canonical_alias
    let canonical_alias = db::room_state::get_event(pool, room_id, "m.room.canonical_alias", "")
        .await
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("alias")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string())
        });

    Ok(serde_json::json!({
        "room_id": room_id,
        "name": name,
        "topic": topic,
        "canonical_alias": canonical_alias,
        "num_joined_members": count.0,
        "world_readable": world_readable,
        "guest_can_join": guest_can_join,
        "join_rule": join_rule,
        "room_type": room_type,
        "children_state": [],
    }))
}

/// ルームの m.space.child 状態イベント一覧を返す。
async fn get_space_children(
    pool: &sqlx::MySqlPool,
    room_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    use sqlx::Row;

    let rows = sqlx::query(
        r#"SELECT e.event_id, e.sender, rs.state_key, e.content, e.created_at
           FROM room_state rs
           JOIN events e ON e.event_id = rs.event_id
           WHERE rs.room_id = ? AND rs.event_type = 'm.space.child'
           ORDER BY e.stream_ordering ASC"#,
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let content_str: String = r.get("content");
            let ts: chrono::NaiveDateTime = r.get("created_at");
            serde_json::json!({
                "type": "m.space.child",
                "state_key": r.get::<String, _>("state_key"),
                "sender": r.get::<String, _>("sender"),
                "origin_server_ts": ts.and_utc().timestamp_millis(),
                "content": serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_default(),
            })
        })
        .collect())
}
