# Handover — v0.7.0 → v0.8.0

## v0.7.0 でやったこと

- **ルームタグ**: `room_tags` テーブル追加（user_id, room_id, tag, order_）。`GET/PUT/DELETE /_matrix/client/v3/user/{userId}/rooms/{roomId}/tags[/{tag}]` を実装。`/sync` の各参加ルームに `account_data.events` として `m.tag` イベントを返す。DB 層は `crates/db/src/room_tags.rs`。
- **フィルター**: `filters` テーブル追加（filter_id AUTO_INCREMENT, user_id, filter JSON）。`POST/GET /_matrix/client/v3/user/{userId}/filter[/{filterId}]` を実装。`/sync?filter=` パラメータに対応（filter_id またはインライン JSON）。`room.timeline.types` によるタイムラインイベント種別フィルタリングを実装。DB 層は `crates/db/src/filters.rs`。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし。再起動時にタイピング状態が消える（Matrix 仕様上は許容範囲） |
| receipts テーブルは .sqlx/ 未登録 | sqlx offline モードでは `sqlx::query()` 非マクロを使用。マクロ移行する場合はテーブル作成後に `cargo sqlx prepare` 実行が必要 |
| room_aliases / presence / unread / room_tags / filters も非マクロ | 同上。新テーブルのため `.sqlx/` にメタデータなし |
| highlight_count は LIKE 検索 | 正確な mention 検出ではなく、`content` に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| filter は room.timeline.types のみ適用 | room.state.types / room.ephemeral.types / account_data フィルターは未対応 |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.8.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **招待フロー改善** — invite → join の UIA / 通知連携
3. **filter の完全対応** — `room.state.types`, `room.ephemeral.types`, `account_data`, `not_types` 等
4. **To-device メッセージ** — `PUT/GET /_matrix/client/v3/sendToDevice/{type}/{txnId}`

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
