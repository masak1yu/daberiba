# Handover — v0.28.0 → v0.29.0

## v0.28.0 でやったこと

- **highlight_count の正確化** (`db/unread.rs`, `server/push_eval.rs`, `server/api/client/events.rs`):
  - `push_eval::actions_highlight()` を新設し、`set_tweak: highlight` を検出。
  - `dispatch_push` でハイライト判定されたイベントを `unread_highlights` テーブルに記録。
  - `db::unread::record_highlight()` 新設 — INSERT IGNORE で冪等な記録。
  - `get_for_room()` の `highlight_count` を LIKE 検索から `unread_highlights` テーブル参照に変更。
  - `schema.sql` に `unread_highlights` テーブルを追加。

- **`POST /search`** (`server/api/client/search.rs`, `db/events.rs`):
  - ユーザーが参加しているルームの `m.room.message` イベントを body フィールドで LIKE 検索。
  - `filter.rooms` で対象ルームを絞り込み可能。`filter.limit` でページサイズ指定（最大 100）。
  - `db::events::search_room_events()` 新設。

- **`/upgrade` の predecessor event_id 修正** (`server/api/client/rooms.rs`):
  - 旧ルームの最終 event_id を `get_room_tip()` で取得して `m.room.create.predecessor.event_id` に設定。
  - 旧ルームにイベントがない場合は空文字列にフォールバック。

- **`db::events::get_stream_ordering()`** 新設:
  - event_id から stream_ordering を取得するユーティリティ関数。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性 |
| /context の state フィールド | 現在は空配列を返している（指定時点のルームスナップショットは未実装） |
| /search はページネーションなし | next_batch は常に null（全件一括取得のみ） |
| unread_highlights の cleanup なし | 既読送信後も行が残る（COUNT は receipts と結合して絞るため実害はない） |

## v0.29.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/search` のページネーション** — next_batch トークンによる続き取得
3. **`/rooms/{roomId}/threads` エンドポイント** — MSC スレッド対応（m.thread rel_type）
4. **`/notifications` エンドポイント** — push notification 履歴の取得
5. **`unread_highlights` の cleanup** — `POST /receipt` 時に古い highlight を削除

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
