# Handover — v0.14.0 → v0.15.0

## v0.14.0 でやったこと

- **PDU 署名検証** (`xmatrix.rs`): `verify_pdu_signatures()` を追加。`send_transaction` / `send_join` で受信 PDU の Ed25519 署名を検証するようになった。`signatures` フィールドを除いたカノニカル JSON に対して origin サーバーの公開鍵（`fed_key_cache` 活用）で検証。

- **Federation `/event` エンドポイント** (`federation/get_event.rs`): `GET /_matrix/federation/v1/event/:event_id` を新規実装。バックフィル対応。`db::events::get_by_id()` で DB から取得して PDU 形式で返す。

- **状態解決 (State Resolution v2 簡易版)**:
  - `events` テーブルに `origin_server_ts BIGINT NULL` を追加
  - `db::events::store_pdu()`: federation PDU 専用の保存関数。INSERT IGNORE でべき等性を保証。状態イベントは `origin_server_ts` が新しい方を採用し、同一 ts なら event_id の辞書順（先着優先）で解決。
  - `send_join` レスポンスに `auth_chain` を追加（`m.room.create` / `m.room.join_rules` / `m.room.power_levels`）。
  - `state_res.rs`: Rust で同じ解決ルールを表現したヘルパー関数（テスト・将来の拡張用）。

- **Room Version** (`rooms` テーブルに `room_version VARCHAR(16) DEFAULT '10'` を追加):
  - `make_join` / `send_join` が DB の `room_version` を参照して返すようになった。

- **Sync フィルタ limit 適用**:
  - `FilterDef` に `timeline_limit` / `state_limit` フィールドを追加。
  - `db::sync::sync()` が `timeline_limit` を受け取り `LIMIT` 句に反映。デフォルト 50 件、フィルターで上書き可能。

- **パフォーマンス改善**:
  - `send_join`: `store_pdu()` 後の 4 独立 DB クエリを `tokio::join!` で並列化。
  - `make_join`: `count_joined_members()` と `get_version()` を並列化。

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
| PDU の auth_events 未保存 | events テーブルに auth_events カラムがないため、完全な状態解決アルゴリズム v2 は不可 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| SERVER_NAME を毎回 env::var で読む | 起動時にキャッシュすべきだが 15 箇所以上に散在している |

## v0.15.0 候補

1. **PDU auth_events の保存** — `events` テーブルに `auth_events TEXT NULL`（JSON 配列）を追加し、auth chain の完全なトラバーサルを可能にする
2. **状態解決アルゴリズム v2 完全実装** — auth_events グラフを使った完全な conflict resolution
3. **Backfill 実装** — `GET /backfill` エンドポイントで過去イベントを取得
4. **Federation send_leave** — `PUT /_matrix/federation/v2/send_leave/:room_id/:event_id`
5. **SERVER_NAME キャッシュ化** — 起動時に一度読んで `AppState` に保持

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
