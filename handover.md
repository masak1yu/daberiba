# Handover — v0.10.0 → v0.11.0

## v0.10.0 でやったこと

- **account_data**: グローバル・ルーム固有の account_data を保存・取得する API を実装。`account_data` テーブル追加（user_id, room_id, event_type, content）。`PUT/GET /_matrix/client/v3/user/{userId}/account_data/{type}` および `PUT/GET /_matrix/client/v3/user/{userId}/rooms/{roomId}/account_data/{type}` を実装。`/sync` のトップレベル `account_data.events`（グローバル）と各ルームの `account_data.events`（ルーム固有 + m.tag）に統合。
- **to_device at-least-once 化**: `/sync` の `next_batch` トークンを `{stream_ordering}_{max_to_device_id}` 形式に変更。`since` トークンから acked_to_device_id を解析し、次回 sync 呼び出し時に `id <= acked_id` のメッセージを削除してから新規メッセージを返す。クライアントが sync 応答を受信失敗した場合でも次回 sync で再取得可能。`db::to_device::delete_acked()` に変更（旧 `delete_delivered()` を置換）。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | receipts / room_aliases / presence / unread / room_tags / filters / to_device / keys / account_data は `sqlx::query()` 非マクロを使用。`cargo sqlx prepare` でマクロ移行可能 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装。サーバーは鍵の保存・中継のみ |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.11.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **E2EE 鍵バックアップ** — `/_matrix/client/v3/room_keys/` エンドポイント（Megolm セッションキーのサーバー保管）
3. **push_rules** — `/_matrix/client/v3/pushrules/` で通知ルール管理（デフォルトルール + ユーザーカスタムルール）
4. **account_data sync since 対応** — `since` がある場合は updated_at > since の差分のみ返すように最適化

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
