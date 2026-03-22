# Handover — v0.6.0 → v0.7.0

## v0.6.0 でやったこと

- **既読数バッジ / 通知カウント**: `unread_notifications`（`notification_count` / `highlight_count`）を `/sync` の各参加ルームに追加。ユーザーの最終 `m.read` レシートの `stream_ordering` 以降のタイムラインイベント数を DB でカウント。`highlight_count` はコンテンツに `user_id` を含むイベント数（LIKE 検索）。DB 層は `crates/db/src/unread.rs`。
- **ルームエイリアス**: `room_aliases` テーブル追加（alias, room_id, creator）。`PUT/GET/DELETE /_matrix/client/v3/directory/room/{roomAlias}` を実装。`POST /join/{roomIdOrAlias}` で `#` で始まる場合はエイリアス解決してから join。DB 層は `crates/db/src/room_aliases.rs`。
- **プレゼンス**: `presence` テーブル追加（user_id, presence, status_msg, last_active_ts）。`PUT/GET /_matrix/client/v3/presence/{userId}/status` を実装。`/sync` の `presence.events` に参加ルームのメンバーの `m.presence` イベントを返す（presence 登録済みのユーザーのみ）。DB 層は `crates/db/src/presence.rs`。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし。再起動時にタイピング状態が消える（Matrix 仕様上は許容範囲） |
| receipts テーブルは .sqlx/ 未登録 | sqlx offline モードでは `sqlx::query()` 非マクロを使用。マクロ移行する場合はテーブル作成後に `cargo sqlx prepare` 実行が必要 |
| room_aliases / presence / unread も非マクロ | 同上。新テーブルのため `.sqlx/` にメタデータなし |
| highlight_count は LIKE 検索 | 正確な mention 検出ではなく、`content` に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.7.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **招待フロー改善** — invite → join の UIA / 通知連携
3. **フィルター** — `/sync` の `filter` パラメータ対応（イベント種別・ルームの絞り込み）
4. **ルームタグ** — `PUT/DELETE /rooms/{roomId}/tags/{tag}`（クライアント UI 整理用）

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
