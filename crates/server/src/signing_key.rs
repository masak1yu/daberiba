/// サーバー署名鍵管理
///
/// 起動時に Ed25519 鍵ペアを生成してメモリに保持する。
/// 鍵は再起動のたびに再生成されるため、永続化は今後の課題。
/// key ID は "ed25519:auto" を使用。
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;

pub struct ServerSigningKey {
    pub key_id: String,
    signing_key: SigningKey,
}

impl ServerSigningKey {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            key_id: "ed25519:auto".to_string(),
            signing_key,
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
