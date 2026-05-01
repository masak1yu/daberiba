# Handover — v1.3.0

## v1.3.0 でやったこと

### マルチプロバイダー SSO 対応

Google・GitHub・Apple の 3 プロバイダーに対応した。

**アーキテクチャ変更:**
- `crates/server/src/sso.rs` を全面刷新。`ProviderConfig` 構造体と `load_providers()` 関数でプロバイダーを動的にロード。環境変数に CLIENT_ID が設定されているプロバイダーだけが有効になる。
- `AppState` の `sso: Option<Arc<SsoConfig>>` を `sso_providers: Arc<Vec<ProviderConfig>>` に変更。
- `crates/db/src/sso.rs`: `sso_states` テーブルに `provider VARCHAR(32)` カラムを追加。`consume_state` が `(redirect_url, provider_id)` のタプルを返すように変更。
- `crates/server/src/api/client/auth.rs`: SSO フロー全体を書き直し。`exchange_code` / `extract_user_info` がプロバイダー種別（OIDC / GitHub）に応じて分岐。

**プロバイダー別の実装ポイント:**
- Google: OIDC Discovery (`https://accounts.google.com`) で URL を自動取得。
- GitHub: OIDC 非対応のため独自実装。`id`（整数）を sub として使用。`Accept: application/json` ヘッダーが必要。
- Apple: OIDC Discovery (`https://appleid.apple.com`)。ES256 JWT をクライアントシークレットとして生成（`jsonwebtoken` クレート追加）。userinfo エンドポイントがないため `id_token` の payload を base64url デコードして sub を抽出。

**環境変数（`.env.example` に追記）:**
```
GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET
GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET
APPLE_CLIENT_ID / APPLE_TEAM_ID / APPLE_KEY_ID / APPLE_PRIVATE_KEY
```

**フロントエンド（`front/src/`）:**
- `api/auth.ts`: `fetchLoginFlows` / `loginWithToken` を追加。
- `pages/LoginPage.tsx`: ホームサーバーが変わるたびにログインフローを取得し、SSO ボタンを動的表示。Google・GitHub・Apple 用のインラインアイコン SVG を追加。SSO コールバック（`?loginToken=`）を受け取って自動ログインするエフェクトを追加。

### バグ修正

**`/publicRooms` で 500 エラー**（`crates/db/src/rooms.rs`）:
- `COUNT(DISTINCT r.room_id)` は MariaDB で `BIGINT`（符号付き `i64`）を返すが、`let total: u64` と型アノテーションしていたため sqlx がランタイムで型不一致エラーを出していた。
- 修正: `let total: i64` に変更し、`let total = total.max(0) as u64;` でキャスト。

**エラーログの改善**（`crates/server/src/error.rs`）:
- `tracing::error!(error = %self)` → `tracing::error!(error = ?self)` に変更。Display ではなく Debug フォーマットを使うことでエラーチェーン全体が出力されるようになった。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| SSO redirect_url ホワイトリスト未検証 | open redirect リスクあり（低優先）|
| Apple SSO は有料 Developer Account が必要 | 個人開発では検証コストが高い |
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events DAG を使った完全な conflict resolution は未実装 |
| 3pid バリデーションなし | identity server 連携なし。登録は直接 INSERT のみ（メール確認なし） |
| /hierarchy の cross-server 展開 | federation ルームの子は room_state に m.space.child がない場合スキップされる |

## 次バージョン以降の候補

特定のロードマップは設定せず、実際に動かしながら出てきた問題を都度対処する方針。

潜在的な候補（優先度低）:
- `rooms.encrypted` フラグを `/publicRooms` や `GET /rooms/{roomId}/summary` に活用
- `restricted` join_rule のフル実装
- receipt の `/sync` で `m.read.private` を非公開配信
- フロントエンド: スレッド表示、リアクション送信 UI、設定画面
- モーダルのアクセシビリティ（Escape キー、`aria-modal`、フォーカストラップ）

## 開発フロー

```sh
# 環境起動（DB）
podman compose up -d db

# スキーマ適用
./dev schema-apply

# サーバー起動（バックエンド）
cargo run --bin server

# フロントエンド起動
cd front && pnpm dev

# テスト
cargo test
cargo clippy --all-targets -- -D warnings

# フォーマット
cargo fmt
cd front && pnpm format
```

## 環境設定

- `.env.example` → `.env` にコピーしてパスワードを設定
- `SQLX_OFFLINE=true` で DB なしビルド可能（`.sqlx/` がコミット済みのため CI でも動作）
- `MEDIA_BACKEND=s3` + `S3_BUCKET` で S3 に切り替え可能（`--features server/s3`）

## ブランチ戦略

- `main` — リリース済みタグのみマージ
- 作業は機能単位でブランチを切り、完成後に main へマージ
