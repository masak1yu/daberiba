use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/_matrix/client/v3/pushrules/", get(get_all_rules))
        .route(
            "/_matrix/client/v3/pushrules/:scope/:kind/:ruleId",
            get(get_rule).put(put_rule).delete(delete_rule),
        )
        .route(
            "/_matrix/client/v3/pushrules/:scope/:kind/:ruleId/enabled",
            get(get_rule_enabled).put(put_rule_enabled),
        )
        .route(
            "/_matrix/client/v3/pushrules/:scope/:kind/:ruleId/actions",
            get(get_rule_actions).put(put_rule_actions),
        )
}

// ─── デフォルトルール ──────────────────────────────────────────────────────────

/// Matrix 仕様で定義されたデフォルトプッシュルールセットを生成
/// user_localpart はコンテンツルール (.m.rule.contains_user_name) のパターンに使用
fn default_rules(user_localpart: &str) -> serde_json::Value {
    serde_json::json!({
        "override": [
            {
                "rule_id": ".m.rule.master",
                "default": true,
                "enabled": false,
                "conditions": [],
                "actions": ["dont_notify"]
            },
            {
                "rule_id": ".m.rule.suppress_notices",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "content.msgtype", "pattern": "m.notice"}],
                "actions": ["dont_notify"]
            },
            {
                "rule_id": ".m.rule.invite_for_me",
                "default": true,
                "enabled": true,
                "conditions": [
                    {"kind": "event_match", "key": "type", "pattern": "m.room.member"},
                    {"kind": "event_match", "key": "content.membership", "pattern": "invite"},
                    {"kind": "event_match", "key": "state_key", "pattern": user_localpart}
                ],
                "actions": ["notify", {"set_tweak": "sound", "value": "default"}, {"set_tweak": "highlight", "value": false}]
            },
            {
                "rule_id": ".m.rule.member_event",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.member"}],
                "actions": ["dont_notify"]
            },
            {
                "rule_id": ".m.rule.contains_display_name",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "contains_display_name"}],
                "actions": ["notify", {"set_tweak": "sound", "value": "default"}, {"set_tweak": "highlight"}]
            },
            {
                "rule_id": ".m.rule.tombstone",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.tombstone"}],
                "actions": ["notify", {"set_tweak": "highlight"}]
            },
            {
                "rule_id": ".m.rule.roomnotif",
                "default": true,
                "enabled": true,
                "conditions": [
                    {"kind": "event_match", "key": "content.body", "pattern": "@room"},
                    {"kind": "sender_notification_permission", "key": "room"}
                ],
                "actions": ["notify", {"set_tweak": "highlight"}]
            }
        ],
        "content": [
            {
                "rule_id": ".m.rule.contains_user_name",
                "default": true,
                "enabled": true,
                "pattern": user_localpart,
                "actions": ["notify", {"set_tweak": "sound", "value": "default"}, {"set_tweak": "highlight"}]
            }
        ],
        "room": [],
        "sender": [],
        "underride": [
            {
                "rule_id": ".m.rule.call",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.call.invite"}],
                "actions": ["notify", {"set_tweak": "sound", "value": "ring"}]
            },
            {
                "rule_id": ".m.rule.encrypted_room_one_to_one",
                "default": true,
                "enabled": true,
                "conditions": [
                    {"kind": "room_member_count", "is": "==2"},
                    {"kind": "event_match", "key": "type", "pattern": "m.room.encrypted"}
                ],
                "actions": ["notify", {"set_tweak": "sound", "value": "default"}]
            },
            {
                "rule_id": ".m.rule.room_one_to_one",
                "default": true,
                "enabled": true,
                "conditions": [
                    {"kind": "room_member_count", "is": "==2"},
                    {"kind": "event_match", "key": "type", "pattern": "m.room.message"}
                ],
                "actions": ["notify", {"set_tweak": "sound", "value": "default"}]
            },
            {
                "rule_id": ".m.rule.message",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.message"}],
                "actions": ["notify"]
            },
            {
                "rule_id": ".m.rule.encrypted",
                "default": true,
                "enabled": true,
                "conditions": [{"kind": "event_match", "key": "type", "pattern": "m.room.encrypted"}],
                "actions": ["notify"]
            }
        ]
    })
}

/// ユーザーの m.push_rules account_data をロード。なければデフォルト。
async fn load_rules(pool: &sqlx::MySqlPool, user_id: &str) -> serde_json::Value {
    if let Ok(Some(v)) = db::account_data::get(pool, user_id, "", "m.push_rules").await {
        if let Some(global) = v.get("global") {
            if global.is_object() {
                return v;
            }
        }
    }
    let localpart = user_id
        .split(':')
        .next()
        .unwrap_or(user_id)
        .trim_start_matches('@');
    serde_json::json!({ "global": default_rules(localpart) })
}

/// ルールセットを保存
async fn save_rules(
    pool: &sqlx::MySqlPool,
    user_id: &str,
    rules: &serde_json::Value,
) -> anyhow::Result<()> {
    let content = serde_json::to_string(rules)?;
    db::account_data::set(pool, user_id, "", "m.push_rules", &content).await
}

// ─── ハンドラ ──────────────────────────────────────────────────────────────────

async fn get_all_rules(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let rules = load_rules(&state.pool, &user.user_id).await;
    Ok(Json(rules))
}

#[derive(Deserialize)]
struct RulePath {
    scope: String,
    kind: String,
    #[serde(rename = "ruleId")]
    rule_id: String,
}

async fn get_rule(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
) -> ApiResult<Json<serde_json::Value>> {
    let rules = load_rules(&state.pool, &user.user_id).await;
    let rule =
        find_rule(&rules, &p.scope, &p.kind, &p.rule_id).ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(rule.clone()))
}

async fn put_rule(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut rules = load_rules(&state.pool, &user.user_id).await;
    let kind_arr = rules["global"][&p.kind]
        .as_array_mut()
        .ok_or(crate::error::AppError::NotFound)?;

    // 既存ルールがあれば上書き、なければ追加
    let mut new_rule = body;
    new_rule["rule_id"] = serde_json::json!(p.rule_id);
    new_rule["default"] = serde_json::json!(false);
    if let Some(pos) = kind_arr.iter().position(|r| r["rule_id"] == p.rule_id) {
        kind_arr[pos] = new_rule;
    } else {
        kind_arr.push(new_rule);
    }

    save_rules(&state.pool, &user.user_id, &rules).await?;
    Ok(Json(serde_json::json!({})))
}

async fn delete_rule(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut rules = load_rules(&state.pool, &user.user_id).await;
    let kind_arr = rules["global"][&p.kind]
        .as_array_mut()
        .ok_or(crate::error::AppError::NotFound)?;

    let before = kind_arr.len();
    kind_arr.retain(|r| r["rule_id"] != p.rule_id);
    if kind_arr.len() == before {
        return Err(crate::error::AppError::NotFound);
    }

    save_rules(&state.pool, &user.user_id, &rules).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_rule_enabled(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
) -> ApiResult<Json<serde_json::Value>> {
    let rules = load_rules(&state.pool, &user.user_id).await;
    let rule =
        find_rule(&rules, &p.scope, &p.kind, &p.rule_id).ok_or(crate::error::AppError::NotFound)?;
    let enabled = rule["enabled"].as_bool().unwrap_or(true);
    Ok(Json(serde_json::json!({ "enabled": enabled })))
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

async fn put_rule_enabled(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
    Json(body): Json<EnabledBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut rules = load_rules(&state.pool, &user.user_id).await;
    let rule = find_rule_mut(&mut rules, &p.scope, &p.kind, &p.rule_id)
        .ok_or(crate::error::AppError::NotFound)?;
    rule["enabled"] = serde_json::json!(body.enabled);
    save_rules(&state.pool, &user.user_id, &rules).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_rule_actions(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
) -> ApiResult<Json<serde_json::Value>> {
    let rules = load_rules(&state.pool, &user.user_id).await;
    let rule =
        find_rule(&rules, &p.scope, &p.kind, &p.rule_id).ok_or(crate::error::AppError::NotFound)?;
    let actions = rule["actions"].clone();
    Ok(Json(serde_json::json!({ "actions": actions })))
}

#[derive(Deserialize)]
struct ActionsBody {
    actions: Vec<serde_json::Value>,
}

async fn put_rule_actions(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(p): Path<RulePath>,
    Json(body): Json<ActionsBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut rules = load_rules(&state.pool, &user.user_id).await;
    let rule = find_rule_mut(&mut rules, &p.scope, &p.kind, &p.rule_id)
        .ok_or(crate::error::AppError::NotFound)?;
    rule["actions"] = serde_json::json!(body.actions);
    save_rules(&state.pool, &user.user_id, &rules).await?;
    Ok(Json(serde_json::json!({})))
}

// ─── ユーティリティ ────────────────────────────────────────────────────────────

fn find_rule<'a>(
    rules: &'a serde_json::Value,
    scope: &str,
    kind: &str,
    rule_id: &str,
) -> Option<&'a serde_json::Value> {
    rules[scope][kind]
        .as_array()?
        .iter()
        .find(|r| r["rule_id"] == rule_id)
}

fn find_rule_mut<'a>(
    rules: &'a mut serde_json::Value,
    scope: &str,
    kind: &str,
    rule_id: &str,
) -> Option<&'a mut serde_json::Value> {
    rules[scope][kind]
        .as_array_mut()?
        .iter_mut()
        .find(|r| r["rule_id"] == rule_id)
}
