use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/client/v1/rooms/:roomId/summary",
        get(room_summary),
    )
}

/// GET /_matrix/client/v1/rooms/:roomId/summary (MSC3266)
/// ルームのサマリー情報を返す。参加前のプレビュー用途にも使える。
async fn room_summary(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // 複数の状態イベントを並列取得
    let (
        name_ev,
        topic_ev,
        avatar_ev,
        join_rules_ev,
        canonical_alias_ev,
        create_ev,
        encryption_ev,
        guest_access_ev,
        num_joined,
        membership,
    ) = tokio::join!(
        db::room_state::get_event(&state.pool, &room_id, "m.room.name", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.topic", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.avatar", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.join_rules", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.canonical_alias", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.create", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.encryption", ""),
        db::room_state::get_event(&state.pool, &room_id, "m.room.guest_access", ""),
        db::rooms::count_joined_members(&state.pool, &room_id),
        db::rooms::get_membership(&state.pool, &room_id, &user.user_id),
    );

    let name = name_ev?
        .as_ref()
        .and_then(|v| v["name"].as_str())
        .map(|s| s.to_string());
    let topic = topic_ev?
        .as_ref()
        .and_then(|v| v["topic"].as_str())
        .map(|s| s.to_string());
    let avatar_url = avatar_ev?
        .as_ref()
        .and_then(|v| v["url"].as_str())
        .map(|s| s.to_string());
    let join_rule = join_rules_ev?
        .as_ref()
        .and_then(|v| v["join_rule"].as_str())
        .unwrap_or("invite")
        .to_string();
    let canonical_alias = canonical_alias_ev?
        .as_ref()
        .and_then(|v| v["alias"].as_str())
        .map(|s| s.to_string());
    let room_type = create_ev?
        .as_ref()
        .and_then(|v| v["type"].as_str())
        .map(|s| s.to_string());
    let encryption = encryption_ev?
        .as_ref()
        .and_then(|v| v["algorithm"].as_str())
        .map(|s| s.to_string());
    let guest_can_join = guest_access_ev?
        .as_ref()
        .and_then(|v| v["guest_access"].as_str())
        == Some("can_join");
    let world_readable = join_rule == "world_readable";
    let num_joined = num_joined?;
    let membership = membership?.unwrap_or_default();

    let mut resp = serde_json::json!({
        "room_id": room_id,
        "num_joined_members": num_joined,
        "join_rule": join_rule,
        "world_readable": world_readable,
        "guest_can_join": guest_can_join,
        "membership": membership,
    });

    if let Some(v) = name {
        resp["name"] = serde_json::json!(v);
    }
    if let Some(v) = topic {
        resp["topic"] = serde_json::json!(v);
    }
    if let Some(v) = avatar_url {
        resp["avatar_url"] = serde_json::json!(v);
    }
    if let Some(v) = canonical_alias {
        resp["canonical_alias"] = serde_json::json!(v);
    }
    if let Some(v) = room_type {
        resp["room_type"] = serde_json::json!(v);
    }
    if let Some(v) = encryption {
        resp["encryption"] = serde_json::json!(v);
    }

    Ok(Json(resp))
}
