# Handover — v0.27.0 → v0.28.0

## v0.27.0 でやったこと

- **パワーレベルチェック** (`db/room_state.rs`, `api/client/rooms.rs`):
  - `get_user_power_level()` — ユーザーのパワーレベルを m.room.power_levels から取得。
  - `get_required_power_level()` — アクション（kick/ban/redact/state_default 等）の必要 PL を取得。
  - kick/ban/unban: 呼び出し元が必要 PL 未満の場合 403 Forbidden を返すよう変更。
  - redact: 自分のイベント以外を redact する場合は redact PL チェックを追加。

- **`POST /rooms/{roomId}/upgrade`** (`api/client/rooms.rs`):
  - state_default PL チェック後、新ルームを作成してバージョンを設定。
  - 新ルームに m.room.create（predecessor 付き）/ join_rules / power_levels / member イベントを保存。
  - 旧ルームに m.room.tombstone を保存して replacement_room を示す。
  - レスポンス: `{ "replacement_room": "<new_room_id>" }`。

- **`POST /account/deactivate`** (`api/client/account.rs`, `db/users.rs`):
  - UIA（m.login.password）でパスワード確認後、全トークンを失効。
  - `db::users::deactivate()` — password_hash を空文字列に設定してログイン不能化。
  - レスポンス: `{ "id_server_unbind_result": "no-support" }`。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は localpart LIKE | display name メンションや push rule の highlight tweak には未対応 |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性 |
| /context の state フィールド | 現在は空配列を返している（指定時点のルームスナップショットは未実装） |
| upgrade の predecessor event_id | m.room.create の predecessor.event_id は空文字列（旧ルームの最終 event_id を取得していない） |

## v0.28.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **push rule の `highlight` tweak** — `dispatch_push` でのハイライト評価結果を `unread_highlights` テーブルに記録して highlight_count を正確化
3. **`/rooms/{roomId}/upgrade` の predecessor event_id** — 旧ルームの最終 event_id を取得して設定
4. **`/search` エンドポイント** — 全文検索（MariaDB LIKE または FTS）
5. **`/rooms/{roomId}/threads` エンドポイント** — MSC スレッド対応

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
