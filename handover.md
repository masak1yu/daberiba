# Handover — v0.8.0 → v0.9.0

## v0.8.0 でやったこと

- **filter 完全対応**: `/sync?filter=` で以下のフィールドを適用するように拡張。`room.rooms` / `room.not_rooms`（ルーム絞り込み）、`room.timeline.types` / `not_types`（タイムライン）、`room.state.types` / `not_types`（ステート）、`room.ephemeral.types` / `not_types`（エフェメラル）、`room.account_data.types` / `not_types`（アカウントデータ）、`presence.types` / `not_types`（プレゼンス）。フィルター解析は `crates/server/src/filter.rs` の `FilterDef` 構造体に集約。
- **To-device メッセージ**: `to_device_messages` テーブル追加（id AUTO_INCREMENT, sender, recipient, device_id, event_type, content, txn_id）。`PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}` を実装。`/sync` レスポンスの `to_device.events` に未配信メッセージを返し、返却後に削除（at-most-once 配信）。DB 層は `crates/db/src/to_device.rs`。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| receipts / room_aliases / presence / unread / room_tags / filters / to_device は非マクロ | sqlx offline モードでは `sqlx::query()` 非マクロを使用。`cargo sqlx prepare` でマクロ移行可能 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| to_device は at-most-once 配信 | sync で返した後に即削除するため、クライアントが受信失敗した場合に再送不可 |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.9.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **招待フロー改善** — invite → join の通知連携（push notification / to-device 活用）
3. **to_device at-least-once 化** — sync next_batch ベースの既読管理に変更
4. **E2EE 鍵管理** — `/_matrix/client/v3/keys/upload` / `keys/query` / `keys/claim`

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
