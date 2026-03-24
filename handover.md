# Handover — v0.15.0 → v0.16.0

## v0.15.0 でやったこと

- **SERVER_NAME キャッシュ化**:
  - `AppState` に `server_name: Arc<str>` フィールドを追加し、起動時に一度だけ env から読む。
  - 従来 15 箇所以上に散在していた `std::env::var("SERVER_NAME")` を `state.server_name` に統一。
  - `db::events::send()` の署名に `server_name: &str` を追加（呼び出し側から渡すように変更）。

- **PDU auth_events の保存**:
  - `schema/schema.sql` の `events` テーブルに `auth_events TEXT NULL`（JSON 配列）カラムを追加。
  - `db::events::PduMeta` に `auth_events: Option<&serde_json::Value>` フィールドを追加。
  - `store_pdu()` が INSERT 時に `auth_events` を保存するように更新。
  - `send_join` / `send_transaction` の `PduMeta` 構築で `auth_events` を渡すように更新。

- **Federation Backfill エンドポイント** (`federation/backfill.rs`):
  - `GET /_matrix/federation/v1/backfill/:room_id` を新規実装。
  - クエリパラメータ `v`（起点 event_id、複数可）と `limit`（最大 100、デフォルト 10）に対応。
  - `db::events::get_backfill()` を追加: 起点 event_id の最小 stream_ordering を取得し、それより古いイベントを降順で返す。

- **Federation send_leave エンドポイント** (`federation/send_leave.rs`):
  - `PUT /_matrix/federation/v2/send_leave/:room_id/:event_id` を新規実装。
  - X-Matrix 署名検証 + PDU Ed25519 署名検証を実施。
  - `m.room.member` / `membership: leave` イベントを DB に保存し、`db::rooms::leave()` を呼び出す。

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
| 状態解決アルゴリズム v2 未完全 | auth_events は DB に保存されるようになったが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |

## v0.16.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events グラフを使った完全な conflict resolution（`auth_events` カラムが揃ったので実装可能に）
2. **make_leave エンドポイント** — `GET /_matrix/federation/v1/make_leave/:room_id/:user_id`（leave イベントテンプレートを返す）
3. **invite エンドポイント（Federation）** — `PUT /_matrix/federation/v2/invite/:room_id/:event_id`
4. **prev_events の連鎖管理** — events テーブルに `prev_events` を保存し、DAG の正確なトラバーサルを可能にする
5. **Federation 送信側の実装** — 他サーバーのルームへの参加時に make_join → send_join を自サーバーから発行する

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
