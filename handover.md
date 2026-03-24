# Handover — v0.16.0 → v0.17.0

## v0.16.0 でやったこと

- **make_leave エンドポイント** (`federation/make_leave.rs`):
  - `GET /_matrix/federation/v1/make_leave/:room_id/:user_id` を新規実装。
  - ルームが存在し参加メンバーがいることを確認してから leave イベントテンプレートを返す。
  - make_join と対称的な実装。

- **Federation invite エンドポイント** (`federation/invite.rs`):
  - `PUT /_matrix/federation/v2/invite/:room_id/:event_id` を新規実装。
  - X-Matrix 署名検証 + PDU Ed25519 署名検証を実施。
  - invitee が自サーバーのローカルユーザーであることを確認（`db::users::exists()` 追加）。
  - ルームが未登録の場合は `db::rooms::ensure_placeholder()` でプレースホルダー挿入。
  - `room_memberships` に invite を記録し、PDU に自サーバーの Ed25519 署名を追加して返す。

- **rooms.creator_user_id を NULL 許容に変更**:
  - `schema/schema.sql` の `creator_user_id VARCHAR(255) NOT NULL` → `NULL`。
  - federation から招待されたルームをプレースホルダーとして登録可能にした。
  - FK は `ON DELETE SET NULL` に変更。

- **prev_events 保存**:
  - `schema/schema.sql` の `events` テーブルに `prev_events TEXT NULL`（JSON 配列）カラムを追加。
  - `db::events::PduMeta` に `prev_events: Option<&serde_json::Value>` フィールドを追加。
  - `store_pdu()` が `prev_events` を保存するように更新。
  - `send_join` / `send_transaction` / `send_leave` の `PduMeta` 構築で `prev_events` を渡すように更新。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決が浅い | auth_events DAG の完全なグラフトラバーサルは未実装。`send_join` の auth_chain は m.room.create/join_rules/power_levels のみ |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるようになったが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| Federation 送信側が未実装 | 自サーバーから他サーバーへの make_join → send_join フローがない |

## v0.17.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **Federation 送信側の実装** — 自サーバーのユーザーが外部ルームに参加する際 make_join → send_join を発行する
3. **Federation notify_push_gateway** — 他サーバーのユーザーへの push 配送（send_transaction の送信）
4. **invite の sync 反映** — federation 経由で invite された場合も `/sync` の `rooms.invite` に出現させる

## 開発フロー（おさらい）

```sh
# 環境起動（DB）
docker compose up -d db
docker compose run --rm migrate

# ホストでサーバ起動
cargo run --bin server

# スキーマ変更時
#   1. schema/schema.sql を編集
#   2. dry-run で確認
./dev schema-dry-run
#   3. 適用
./dev schema-apply

# sqlx offline 用メタデータ再生成（クエリ変更時）
DATABASE_URL=... cargo sqlx prepare --workspace

# テスト
SQLX_OFFLINE=true cargo test

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
