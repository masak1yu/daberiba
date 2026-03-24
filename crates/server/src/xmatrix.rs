/// X-Matrix 認証の解析と署名検証
///
/// Matrix Federation 仕様 §4.1 に基づく。
/// リモートサーバーの公開鍵を /_matrix/key/v2/server から取得してキャッシュし、
/// Ed25519 署名を検証する。
use crate::{error::AppError, signing_key::canonical_json, state::AppState};
use anyhow::Result;
use axum::http::{HeaderMap, Uri};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};

/// Authorization ヘッダーから解析した X-Matrix クレーム
#[derive(Debug, Clone)]
pub struct XMatrixClaims {
    pub origin: String,
    pub destination: String,
    pub key_id: String,
    pub sig: String,
}

/// "X-Matrix origin="...",destination="...",key="...",sig="..." を解析する
pub fn parse(header: &str) -> Option<XMatrixClaims> {
    let value = header.strip_prefix("X-Matrix ")?;
    let mut origin = None;
    let mut destination = None;
    let mut key_id = None;
    let mut sig = None;

    for part in value.split(',') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("origin=") {
            origin = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = part.strip_prefix("destination=") {
            destination = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = part.strip_prefix("key=") {
            key_id = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = part.strip_prefix("sig=") {
            sig = Some(v.trim_matches('"').to_string());
        }
    }

    Some(XMatrixClaims {
        origin: origin?,
        destination: destination?,
        key_id: key_id?,
        sig: sig?,
    })
}

/// リモートサーバーの公開鍵を取得する（fed_key_cache から優先）
async fn fetch_verifying_key(
    state: &AppState,
    server_name: &str,
    key_id: &str,
) -> Result<VerifyingKey> {
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    let cache_key = format!("{server_name}/{key_id}");

    // キャッシュチェック（ロックを早期解放するためすぐに clone）
    let cached: Option<Vec<u8>> = {
        let entry = state.fed_key_cache.get(&cache_key);
        match entry {
            Some(ref e) if now_ms < e.1 => Some(e.0.clone()),
            _ => None,
        }
    };
    if let Some(key_bytes) = cached {
        let arr: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("cached key has invalid length"))?;
        return Ok(VerifyingKey::from_bytes(&arr)?);
    }

    // キャッシュミスまたは期限切れ: リモートから取得
    let url = format!("https://{}/_matrix/key/v2/server", server_name);
    let resp: serde_json::Value = state.http.get(&url).send().await?.json().await?;

    let valid_until = resp["valid_until_ts"]
        .as_u64()
        .unwrap_or(now_ms + 86_400_000);
    let key_b64 = resp["verify_keys"][key_id]["key"].as_str().ok_or_else(|| {
        anyhow::anyhow!(
            "key {key_id} not found in /_matrix/key/v2/server response from {server_name}"
        )
    })?;

    let key_bytes = STANDARD_NO_PAD.decode(key_b64)?;
    state
        .fed_key_cache
        .insert(cache_key, (key_bytes.clone(), valid_until));

    let arr: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("remote key has invalid length"))?;
    Ok(VerifyingKey::from_bytes(&arr)?)
}

/// X-Matrix Authorization ヘッダーを検証する。
///
/// - method: HTTP メソッド ("GET", "PUT" 等)
/// - uri: リクエスト URI（パス + クエリ文字列）
/// - body: リクエストボディの JSON（GET の場合は None）
///
/// 成功時は解析済みクレームを返す。
pub async fn verify(
    state: &AppState,
    authorization: &str,
    method: &str,
    uri: &str,
    body: Option<&serde_json::Value>,
) -> Result<XMatrixClaims> {
    let claims = parse(authorization).ok_or_else(|| anyhow::anyhow!("invalid X-Matrix header"))?;

    let our_server = &*state.server_name;
    if claims.destination != our_server {
        anyhow::bail!(
            "X-Matrix destination mismatch: expected {our_server}, got {}",
            claims.destination
        );
    }

    // 署名対象オブジェクトを構築してカノニカル JSON に変換
    let mut signed_obj = serde_json::json!({
        "method": method,
        "uri": uri,
        "origin": claims.origin,
        "destination": claims.destination,
    });
    if let Some(b) = body {
        signed_obj["content"] = b.clone();
    }
    let canonical = canonical_json(&signed_obj);

    // 署名をデコード
    let sig_bytes = STANDARD_NO_PAD
        .decode(&claims.sig)
        .map_err(|e| anyhow::anyhow!("invalid signature base64: {e}"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid signature length"))?;
    let signature = Signature::from_bytes(&sig_arr);

    // 公開鍵を取得して検証
    let verifying_key = fetch_verifying_key(state, &claims.origin, &claims.key_id).await?;
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(canonical.as_bytes(), &signature)
        .map_err(|e| anyhow::anyhow!("signature verification failed: {e}"))?;

    Ok(claims)
}

/// PDU の署名を検証する。
///
/// origin サーバーの署名が少なくとも 1 つ有効であれば Ok を返す。
/// signatures フィールドがない、または origin サーバーの有効な署名が見つからない場合はエラー。
pub async fn verify_pdu_signatures(
    state: &AppState,
    pdu: &serde_json::Value,
    origin: &str,
) -> Result<()> {
    let signatures = pdu["signatures"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("PDU に signatures フィールドがありません"))?;

    let origin_sigs = signatures
        .get(origin)
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("{origin} の署名が PDU に見つかりません"))?;

    // signatures / unsigned を除いた PDU のカノニカル JSON を構築
    let mut redacted = pdu.clone();
    if let Some(obj) = redacted.as_object_mut() {
        obj.remove("signatures");
        obj.remove("unsigned");
    }
    let canonical = canonical_json(&redacted);

    // origin サーバーの署名をすべて試し、1 つでも成功すれば OK
    let mut last_err: Option<anyhow::Error> = None;
    for (key_id, sig_val) in origin_sigs {
        let sig_b64 = match sig_val.as_str() {
            Some(s) => s,
            None => continue,
        };
        let sig_bytes = match STANDARD_NO_PAD.decode(sig_b64) {
            Ok(b) => b,
            Err(e) => {
                last_err = Some(anyhow::anyhow!("base64 デコード失敗 ({key_id}): {e}"));
                continue;
            }
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => {
                last_err = Some(anyhow::anyhow!("署名長が不正 ({key_id})"));
                continue;
            }
        };
        let signature = Signature::from_bytes(&sig_arr);

        match fetch_verifying_key(state, origin, key_id).await {
            Ok(verifying_key) => {
                use ed25519_dalek::Verifier;
                match verifying_key.verify(canonical.as_bytes(), &signature) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        last_err = Some(anyhow::anyhow!("署名検証失敗 ({key_id}): {e}"));
                    }
                }
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!("公開鍵取得失敗 ({key_id}): {e}"));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{origin} の有効な署名が PDU に見つかりません")))
}

/// federation ハンドラ向けの簡易ラッパー。
/// Authorization ヘッダーを取り出して検証し、失敗時は `AppError::Unauthorized` を返す。
pub async fn verify_request(
    state: &AppState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    body: Option<&serde_json::Value>,
) -> Result<XMatrixClaims, AppError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    verify(state, auth, method, &uri.to_string(), body)
        .await
        .map_err(|_| AppError::Unauthorized)
}
