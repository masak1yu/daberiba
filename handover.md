# Handover — v0.9.0 → v0.10.0

## v0.9.0 でやったこと

- **招待フロー改善**: `room_memberships` に `invited_by VARCHAR(255) NULL` カラムを追加。`db::rooms::invite()` が招待者 user_id を保存するように変更。`db::rooms::invited_rooms()` 追加。`/sync` の `rooms.invite` に stripped state（m.room.name / m.room.member）を含めるよう対応。招待時に被招待者の HTTP pusher へ非同期 push 通知を送信（ベストエフォート）。
- **E2EE 鍵管理**: `device_keys`（user_id, device_id, key_json）・`one_time_keys`（id AUTO_INCREMENT, user_id, device_id, key_id, key_json）テーブルを追加。DB 層 `crates/db/src/keys.rs`（upload_device_keys / upload_one_time_keys / get_device_keys / claim_one_time_key / count_one_time_keys）実装。API 層 `crates/server/src/api/client/keys.rs`（POST /keys/upload / /keys/query / /keys/claim）実装。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| receipts / room_aliases / presence / unread / room_tags / filters / to_device / keys は非マクロ | sqlx offline モードでは `sqlx::query()` 非マクロを使用。`cargo sqlx prepare` でマクロ移行可能 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| to_device は at-most-once 配信 | sync で返した後に即削除するため、クライアントが受信失敗した場合に再送不可 |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装。サーバーは鍵の保存・中継のみ |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.10.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **to_device at-least-once 化** — sync next_batch ベースの既読管理に変更
3. **E2EE 鍵バックアップ** — `/_matrix/client/v3/room_keys/` エンドポイント（Megolm セッションキーのサーバー保管）
4. **account_data** — `PUT/GET /_matrix/client/v3/user/{userId}/account_data/{type}` でクライアントデータ保存

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

- `main` — リリース済みタグのみマージ（マージ後 release.yml が自動でタグ・GitHub Release・次バージョンブランチを作成）
- `feature/v0.x.0` — バージョン単位の作業ブランチ
- 機能単位でさらに feature ブランチを切っても良い
