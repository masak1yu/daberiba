# Handover — v0.3.0 → v0.4.0

## v0.3.0 でやったこと

- **devices last_seen 更新**: 認証ミドルウェアで `tokio::spawn` による非同期更新。IP は `X-Real-IP` → `X-Forwarded-For` → `ConnectInfo` の優先順で取得。`main.rs` に `into_make_service_with_connect_info` を追加
- **sqlx `query!` マクロ移行**: `crates/db/src/` 全8ファイルをコンパイル時型チェック対応に移行。`Device`・`MediaRecord` に `#[derive(sqlx::FromRow)]` 追加。`.sqlx/` をコミット済み（DB なしビルド対応）。`sync.rs` の `stream_ordering` を `u64` に統一
- **UIA（User Interactive Authentication）**: `m.login.password` ステージのみ対応。`POST /account/password` と `POST /delete_devices` に適用。`uia.rs` モジュール新規追加。`db::users::verify` 追加（パスワード検証のみ）
- **メディア S3 対応**: `S3Store` 実装（`--features server/s3`）。`MEDIA_BACKEND=s3` + `S3_BUCKET` 環境変数で切り替え。MinIO 等の S3 互換ストレージも `AWS_ENDPOINT_URL` で対応

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA セッション管理なし | session ID を発行するだけで検証していない。本来は有効期限付きセッションをメモリ or DB で管理すべき |
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |
| メディアのアクセス制御なし | 現状は認証ユーザー全員がダウンロード可能 |

## v0.4.0 でやること（候補）

### 1. Push Notification
- `POST /_matrix/client/v3/pushers/set`
- pusher テーブル追加・FCM/APNs 送信は外部サービス依存

### 2. UIA セッション管理強化
- session ID の有効期限チェック（5分 TTL 等）
- `auth_sessions` テーブル or インメモリ（DashMap）で管理

### 3. メディアのアクセス制御
- ルーム参加者のみダウンロード可能にする
- `media` テーブルに `room_id` 関連付けか、ダウンロード時にメンバーチェック

### 4. Pagination（`/messages` の `from`/`to` トークン）
- 現在 `LIMIT` のみで cursor ベースのページネーションが未実装

## 開発フロー（おさらい）

```sh
# 環境起動（DB）
docker compose up -d db
docker compose run --rm migrate

# ホストでサーバ起動
cargo run --bin server

# S3 ビルド（MinIO 等）
cargo build --features server/s3

# スキーマ変更時
#   1. schema/schema.sql を編集
#   2. dry-run で確認
./dev schema-dry-run
#   3. 適用
./dev schema-apply

# sqlx offline 用メタデータ再生成（クエリ変更時）
DATABASE_URL=... cargo sqlx prepare --workspace

# テスト
cargo test

# フォーマット
cargo fmt
```

## 環境設定

- `.env.example` → `.env` にコピーしてパスワードを設定（`.env` は gitignore 済み）
- `DB_ROOT_PASS` が必須（MariaDB コンテナ起動時）
- `MEDIA_BACKEND=s3` + `S3_BUCKET` で S3 に切り替え可能（`--features server/s3` でビルド）
- `SQLX_OFFLINE=true` で DB なしビルド可能（`.sqlx/` がコミット済みのため CI でも動作）

## ブランチ戦略

- `main` — リリース済みタグのみマージ
- `feature/v0.x.0` — バージョン単位の作業ブランチ
- 機能単位でさらに feature ブランチを切っても良い
