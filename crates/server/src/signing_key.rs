/// サーバー署名鍵管理
///
/// 起動時に DB から鍵ペアを読み込む。存在しない場合は生成して DB に保存する。
/// DB が利用不可の場合はエフェメラル鍵にフォールバックする（テスト環境等で有用）。
/// key ID は "ed25519:auto" を使用。
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;

pub struct ServerSigningKey {
    pub key_id: String,
    signing_key: SigningKey,
}

impl ServerSigningKey {
    /// DB から鍵を読み込む。存在しない場合は新規生成して DB に保存する。
    /// DB が利用不可の場合はエフェメラル鍵にフォールバックし、警告ログを出す。
    pub async fn load_or_generate(pool: &sqlx::MySqlPool) -> Self {
        const KEY_ID: &str = "ed25519:auto";
        match db::server_signing_key::load(pool, KEY_ID).await {
            Ok(Some(b64)) => {
                if let Ok(bytes) = STANDARD_NO_PAD.decode(&b64) {
                    if let Ok(arr) = <[u8; 32]>::try_from(bytes) {
                        return Self {
                            key_id: KEY_ID.to_string(),
                            signing_key: SigningKey::from_bytes(&arr),
                        };
                    }
                }
                tracing::warn!("DB の署名鍵データが不正です。エフェメラル鍵を使用します");
            }
            Ok(None) => {
                let signing_key = SigningKey::generate(&mut OsRng);
                let b64 = STANDARD_NO_PAD.encode(signing_key.to_bytes());
                if let Err(e) = db::server_signing_key::save(pool, KEY_ID, &b64).await {
                    tracing::warn!("署名鍵を DB に保存できませんでした: {e}");
                }
                return Self {
                    key_id: KEY_ID.to_string(),
                    signing_key,
                };
            }
            Err(e) => {
                tracing::warn!("DB から署名鍵を読み込めません。エフェメラル鍵を使用します: {e}");
            }
        }
        Self {
            key_id: KEY_ID.to_string(),
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    /// 公開鍵を unpadded base64 でエンコードして返す
    pub fn public_key_base64(&self) -> String {
        STANDARD_NO_PAD.encode(self.signing_key.verifying_key().as_bytes())
    }

    /// バイト列に署名し、unpadded base64 で返す
    pub fn sign(&self, data: &[u8]) -> String {
        let sig = self.signing_key.sign(data);
        STANDARD_NO_PAD.encode(sig.to_bytes())
    }
}

/// room version 3+ の event_id を計算する。
///
/// `signatures`, `unsigned`, `event_id` を除いたカノニカル JSON の SHA-256 を
/// URL-safe unpadded base64 でエンコードして `$` を付けたものが event_id。
pub fn compute_event_id(event: &serde_json::Value) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use sha2::{Digest, Sha256};

    let mut redacted = event.clone();
    if let Some(obj) = redacted.as_object_mut() {
        obj.remove("signatures");
        obj.remove("unsigned");
        obj.remove("event_id");
        obj.remove("hashes");
    }
    let canonical = canonical_json(&redacted);
    let hash = Sha256::digest(canonical.as_bytes());
    format!("${}", URL_SAFE_NO_PAD.encode(hash))
}

/// Matrix 仕様のカノニカル JSON（キーをソートして余分な空白なし）
pub fn canonical_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => {
            let sorted: std::collections::BTreeMap<&str, &serde_json::Value> =
                map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            let pairs: Vec<String> = sorted
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}
