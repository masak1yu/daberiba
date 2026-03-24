/// Federation 送信側クライアント
///
/// ローカルユーザーが他サーバーのルームに参加する際の
/// make_join → sign → send_join フローを実装する。
use crate::state::AppState;
use anyhow::{anyhow, Result};

/// room_id から送信先サーバー名を取り出す。
/// `!opaque:server_name` の `server_name` 部分を返す。
pub fn server_from_room_id(room_id: &str) -> Option<&str> {
    room_id.split(':').nth(1)
}

/// 自サーバーのルームかどうかを判定する。
pub fn is_local_room(state: &AppState, room_id: &str) -> bool {
    server_from_room_id(room_id)
        .map(|s| s == &*state.server_name)
        .unwrap_or(true)
}

/// Federation join フロー: make_join → 署名 → send_join → 状態を保存。
///
/// 1. リモートサーバーに GET make_join を送り、join イベントテンプレートを取得する。
/// 2. テンプレートに自サーバーの Ed25519 署名を付与し event_id を計算する。
/// 3. リモートサーバーに PUT send_join を送る。
/// 4. レスポンスのルーム状態を DB に保存し、メンバーシップを 'join' に更新する。
pub async fn join_remote_room(state: &AppState, room_id: &str, user_id: &str) -> Result<()> {
    let remote_server =
        server_from_room_id(room_id).ok_or_else(|| anyhow!("invalid room_id: {room_id}"))?;

    // 1. make_join
    let make_join_uri = format!(
        "/_matrix/federation/v1/make_join/{}/{}",
        url::form_urlencoded::byte_serialize(room_id.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(user_id.as_bytes()).collect::<String>()
    );
    let make_join_url = format!("https://{}{}", remote_server, make_join_uri);
    let auth = crate::xmatrix::make_auth_header(state, remote_server, "GET", &make_join_uri, None);

    let resp: serde_json::Value = state
        .http
        .get(&make_join_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| anyhow!("make_join HTTP error: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("make_join parse error: {e}"))?;

    let mut event = resp["event"]
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("make_join: missing event"))?;

    // now_ms を設定
    event.insert(
        "origin_server_ts".to_string(),
        serde_json::Value::Number(chrono::Utc::now().timestamp_millis().into()),
    );
    event.insert(
        "origin".to_string(),
        serde_json::Value::String(state.server_name.to_string()),
    );

    // 2. 署名を付与
    let event_val = serde_json::Value::Object(event.clone());
    let mut event_for_signing = event.clone();
    event_for_signing.remove("signatures");
    let canonical =
        crate::signing_key::canonical_json(&serde_json::Value::Object(event_for_signing));
    let sig = state.signing_key.sign(canonical.as_bytes());
    let key_id = &state.signing_key.key_id;

    event
        .entry("signatures")
        .or_insert_with(|| serde_json::json!({}));
    event["signatures"][&*state.server_name][key_id] = serde_json::Value::String(sig);

    // event_id を計算（room version 3+）
    let event_id = crate::signing_key::compute_event_id(&event_val);
    event.insert(
        "event_id".to_string(),
        serde_json::Value::String(event_id.clone()),
    );

    let signed_event = serde_json::Value::Object(event);

    // 3. send_join
    let send_join_uri = format!(
        "/_matrix/federation/v2/send_join/{}/{}",
        url::form_urlencoded::byte_serialize(room_id.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(event_id.as_bytes()).collect::<String>()
    );
    let send_join_url = format!("https://{}{}", remote_server, send_join_uri);
    let auth = crate::xmatrix::make_auth_header(
        state,
        remote_server,
        "PUT",
        &send_join_uri,
        Some(&signed_event),
    );

    let join_resp: serde_json::Value = state
        .http
        .put(&send_join_url)
        .header("Authorization", &auth)
        .json(&signed_event)
        .send()
        .await
        .map_err(|e| anyhow!("send_join HTTP error: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("send_join parse error: {e}"))?;

    // 4. 状態を DB に保存
    // ルームが DB に存在しない場合はプレースホルダーを作成
    db::rooms::ensure_placeholder(&state.pool, room_id).await?;

    // メンバーシップを join に更新
    db::rooms::join(&state.pool, user_id, room_id).await?;

    // ルームの room_version を保存
    if let Some(rv) = join_resp["room_version"].as_str() {
        db::rooms::set_version(&state.pool, room_id, rv).await?;
    }

    // state PDU をすべて保存
    if let Some(state_events) = join_resp["state"].as_array() {
        for pdu in state_events {
            store_state_pdu(state, pdu).await;
        }
    }

    // auth_chain PDU を保存
    if let Some(auth_chain) = join_resp["auth_chain"].as_array() {
        for pdu in auth_chain {
            store_state_pdu(state, pdu).await;
        }
    }

    Ok(())
}

/// 単一の state PDU を保存する（エラーは警告ログに留める）。
async fn store_state_pdu(state: &AppState, pdu: &serde_json::Value) {
    let event_id = match pdu["event_id"].as_str() {
        Some(id) => id,
        None => return,
    };
    let room_id = match pdu["room_id"].as_str() {
        Some(id) => id,
        None => return,
    };
    let sender = pdu["sender"].as_str().unwrap_or("");
    let event_type = pdu["type"].as_str().unwrap_or("");
    let state_key = pdu["state_key"].as_str();
    let content = pdu.get("content").cloned().unwrap_or_default();
    let origin_server_ts = pdu["origin_server_ts"]
        .as_i64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let auth_events = pdu.get("auth_events");
    let prev_events = pdu.get("prev_events");

    if let Err(e) = db::events::store_pdu(
        &state.pool,
        &db::events::PduMeta {
            event_id,
            room_id,
            sender,
            event_type,
            state_key,
            content: &content,
            auth_events,
            prev_events,
            origin_server_ts,
        },
    )
    .await
    {
        tracing::warn!(event_id, error = %e, "federation join: state PDU 保存失敗");
    }
}
