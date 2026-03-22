# Handover — v0.5.0 → v0.6.0

## v0.5.0 でやったこと

- **Read Receipts**: `receipts` テーブル追加（room_id, user_id, receipt_type, event_id, ts）。`POST /rooms/{roomId}/receipt/{receiptType}/{eventId}` で upsert。`/sync` レスポンスの `ephemeral.events` に `m.receipt` イベントとして返す。DB 層は `sqlx::query()` 非マクロで実装（テーブルが新規のため）。
- **タイピングインジケータ**: `TypingStore`（DashMap + Instant TTL）をインメモリで導入。`AppState` に `typing: Arc<TypingStore>` 追加。`PUT /rooms/{roomId}/typing/{userId}` で set/unset（`{"typing": true, "timeout": 30000}`）。`/sync` の `ephemeral.events` に `m.typing` イベントを返す（タイピング中ユーザーが 0 人でも常に含む）。
- **パブリックルーム一覧**: `GET /publicRooms` を実装。`room_state` の `m.room.join_rules` が `"public"` なルームを JSON_EXTRACT で抽出し、参加メンバー数とともに返す。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし。再起動時にタイピング状態が消える（Matrix 仕様上は許容範囲） |
| receipts テーブルは .sqlx/ 未登録 | sqlx offline モードでは `sqlx::query()` 非マクロを使用。マクロ移行する場合はテーブル作成後に `cargo sqlx prepare` 実行が必要 |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.6.0 候補

1. **既読数バッジ / 通知カウント** — `/sync` の `unread_notifications` フィールド（highlight_count / notification_count）
2. **ルームエイリアス** — `PUT/GET/DELETE /_matrix/client/v3/directory/room/{roomAlias}`
3. **プレゼンス** — `PUT /presence/{userId}/status` + `/sync` の `presence.events`
4. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）

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
