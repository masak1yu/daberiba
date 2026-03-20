# Handover — v0.1.0 → v0.2.0

## v0.1.0 でやったこと

- Rust (Axum + sqlx) + MariaDB によるサーバ骨格
- Client-Server API Phase 1 実装（認証・ルーム・イベント・sync・プロフィール）
- argon2 パスワードハッシュ
- sqldef (mysqldef) によるスキーマ管理
- just + mysqldef を同梱したツールコンテナ（`Dockerfile.tools`）
- `./dev` スクリプト（just 未インストールでも動作）
- compose.yml に migrate サービス（`podman compose up` で自動スキーマ適用）
- CORS_ORIGINS 環境変数による CORS 制御
- 認証エンドポイントの入力バリデーション
- スモークテスト 8件

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| sqlx compile-time check 未使用 | 現在は runtime query()。DB 起動後に `cargo sqlx prepare` を走らせ `.sqlx/` をコミットすると型安全になる |
| sync の next_batch がタイムスタンプ依存 | ミリ秒精度で概ね動くが、同一ms内の複数イベントで取りこぼし可能性あり。`events` テーブルに `stream_ordering BIGINT AUTO_INCREMENT` を追加して cursor として使う方が堅牢 |
| パスワード変更 API 未実装 | `POST /_matrix/client/v3/account/password` |
| デバイス管理 API 未実装 | `GET/DELETE /_matrix/client/v3/devices` |
| メディア API 未実装 | `/_matrix/media/v3/upload` など |
| ログアウト時の token revoke のみ | セッション単位でなくデバイスと token を紐付けた管理に整備が必要 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |
| ローカル MySQL と port 競合 | 13306 を使用中。`.env` の `DB_PORT` で変更可能 |

## v0.2.0 でやること（優先順）

### 1. sync の cursor 改善（stream_ordering）
```sql
-- schema/schema.sql に追加
ALTER TABLE events ADD COLUMN stream_ordering BIGINT AUTO_INCREMENT UNIQUE AFTER event_id;
```
`since` / `next_batch` を stream_ordering ベースに変更。

### 2. デバイス管理
- `GET /_matrix/client/v3/devices`
- `GET /_matrix/client/v3/devices/{deviceId}`
- `PUT /_matrix/client/v3/devices/{deviceId}`
- `DELETE /_matrix/client/v3/devices/{deviceId}`
- `POST /_matrix/client/v3/delete_devices`

### 3. パスワード変更
- `POST /_matrix/client/v3/account/password`

### 4. メディア API（基本）
- `POST /_matrix/media/v3/upload`
- `GET /_matrix/media/v3/download/{serverName}/{mediaId}`
- ストレージはローカルファイルシステム or S3 互換（未決定）

### 5. sqlx offline mode 整備
```sh
DATABASE_URL=... cargo sqlx prepare
git add .sqlx
```
以降 DB なしでも `cargo build` が型チェック付きで通る。

### 6. Push Notification（任意）
- `POST /_matrix/client/v3/pushers/set`

## 開発フロー（おさらい）

```sh
# 環境起動（DB + スキーマ自動適用）
podman compose up -d db migrate

# ホストでサーバ起動
cargo run --bin server

# スキーマ変更時
#   1. schema/schema.sql を編集
#   2. dry-run で確認
./dev schema-dry-run
#   3. 適用
./dev schema-apply

# テスト
cargo test
```

## ブランチ戦略

- `main` — リリース済みタグのみマージ
- `v0.x.0` — バージョン単位の作業ブランチ
- 機能単位でさらに feature ブランチを切っても良い
