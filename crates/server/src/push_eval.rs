//! push rule 評価モジュール
//! Matrix spec: https://spec.matrix.org/v1.2/client-server-api/#push-rules
//!
//! イベントに対してユーザーのプッシュルールを評価し、
//! マッチしたルールのアクション一覧を返す。
//! マッチするルールがなければ None を返す。

/// glob パターンマッチ（Matrix spec: * = 任意の文字列、? = 1文字）
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some(&'*'), _) => {
            // * は 0 文字以上にマッチ
            glob_match_inner(&p[1..], t) || (!t.is_empty() && glob_match_inner(p, &t[1..]))
        }
        (Some(&'?'), Some(_)) => glob_match_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if pc == tc => glob_match_inner(&p[1..], &t[1..]),
        _ => false,
    }
}

/// ネストしたキーでイベントフィールドを取得（例: "content.body"）
fn get_field<'a>(event: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut current = event;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

/// room_member_count 条件を評価する
/// is フォーマット: "==2", ">2", "<2", ">=2", "<=2"
fn eval_member_count(is: &str, count: u64) -> bool {
    let (op, num_str) = if let Some(s) = is.strip_prefix("==") {
        ("==", s)
    } else if let Some(s) = is.strip_prefix(">=") {
        (">=", s)
    } else if let Some(s) = is.strip_prefix("<=") {
        ("<=", s)
    } else if let Some(s) = is.strip_prefix('>') {
        (">", s)
    } else if let Some(s) = is.strip_prefix('<') {
        ("<", s)
    } else {
        return false;
    };
    let Ok(n) = num_str.parse::<u64>() else {
        return false;
    };
    match op {
        "==" => count == n,
        ">=" => count >= n,
        "<=" => count <= n,
        ">" => count > n,
        "<" => count < n,
        _ => false,
    }
}

/// 1 つのルールの条件を評価する
fn eval_conditions(
    conditions: &serde_json::Value,
    event: &serde_json::Value,
    member_count: u64,
    recipient_display_name: Option<&str>,
) -> bool {
    let Some(conds) = conditions.as_array() else {
        return true; // conditions なし → 常にマッチ
    };
    for cond in conds {
        let kind = cond["kind"].as_str().unwrap_or("");
        match kind {
            "event_match" => {
                let key = cond["key"].as_str().unwrap_or("");
                let pattern = cond["pattern"].as_str().unwrap_or("*");
                let field_val = get_field(event, key).and_then(|v| v.as_str()).unwrap_or("");
                if !glob_match(pattern, field_val) {
                    return false;
                }
            }
            "contains_display_name" => {
                if let Some(name) = recipient_display_name {
                    if name.is_empty() {
                        return false;
                    }
                    let body = event["content"]["body"].as_str().unwrap_or("");
                    if !body.to_lowercase().contains(&name.to_lowercase()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "room_member_count" => {
                let is = cond["is"].as_str().unwrap_or("");
                if !eval_member_count(is, member_count) {
                    return false;
                }
            }
            "sender_notification_permission" => {
                // 簡易実装: 常に許可
            }
            _ => {
                // 未知の condition → マッチしない
                return false;
            }
        }
    }
    true
}

/// content ルール（pattern フィールドあり）を評価する
fn eval_content_rule(rule: &serde_json::Value, event: &serde_json::Value) -> bool {
    let pattern = rule["pattern"].as_str().unwrap_or("");
    if pattern.is_empty() {
        return false;
    }
    let body = event["content"]["body"].as_str().unwrap_or("");
    glob_match(pattern, body)
}

/// ルールセットに対してイベントを評価し、マッチしたルールのアクションを返す。
/// どのルールにもマッチしなければ None を返す。
pub fn eval_push_rules(
    rules: &serde_json::Value,
    event: &serde_json::Value,
    member_count: u64,
    recipient_display_name: Option<&str>,
) -> Option<Vec<serde_json::Value>> {
    let global = &rules["global"];

    // 評価順: override → content → room → sender → underride
    for kind in &["override", "content", "room", "sender", "underride"] {
        let Some(arr) = global[kind].as_array() else {
            continue;
        };
        for rule in arr {
            if !rule["enabled"].as_bool().unwrap_or(true) {
                continue;
            }
            let matched = if *kind == "content" {
                eval_content_rule(rule, event)
            } else {
                eval_conditions(
                    &rule["conditions"],
                    event,
                    member_count,
                    recipient_display_name,
                )
            };
            if matched {
                if let Some(actions) = rule["actions"].as_array() {
                    return Some(actions.clone());
                }
                return Some(vec![]);
            }
        }
    }
    None
}

/// アクション一覧に "notify" が含まれるかどうか
pub fn actions_notify(actions: &[serde_json::Value]) -> bool {
    actions.iter().any(|a| a.as_str() == Some("notify"))
}
