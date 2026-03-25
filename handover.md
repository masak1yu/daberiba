# Handover — v0.32.0 → v0.33.0

## v0.32.0 でやったこと

- **`GET /rooms/{roomId}/threads`** (`server/api/client/threads.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/threads` — スレッドルート一覧を最新活動順で返す。
  - `event_relations` テーブルの `rel_type = 'm.thread'` を集計してスレッドルートを特定。
  - `?from=<stream_ordering>&limit=<n>` カーソルページネーション。
  - `?include=participated` フィルタ — 自分が投稿したスレッドのみ返す。
  - 各イベントの `unsigned.m.relations.m.thread` に `count` / `latest_event` / `current_user_participated` を付与。

- **`GET /rooms/{roomId}/aliases`** (`server/api/client/room_aliases.rs`):
  - `/_matrix/client/v3/rooms/{roomId}/aliases` — ルームに紐づくエイリアス一覧を返す。
  - `db::room_aliases::list_for_room()` 新設（ORDER BY alias 昇順）。

## v0.31.0 でやったこと

- **`unsigned.m.relations` 集計** (`db/relations.rs`, `db/events.rs`):
  - `relations::get_aggregations_batch(pool, event_ids)` を新設。複数 event_id を一括で集計。
  - **m.replace**: 最新の置換イベント（event_id / sender / origin_server_ts）を返す。
  - **m.reaction**: emoji key ごとの件数リスト `{"chunk": [{"type": "m.reaction", "key": "👍", "count": 3}]}` を返す。
  - `db::events::get_by_id()` — `unsigned.m.relations` を付与して返す。
  - `db::events::get_messages()` — 全イベントを一括集計して `unsigned.m.relations` を付与。
  - `db::events::get_context()` — 前後イベントに同様の集計を付与。

## v0.30.0 でやったこと

- **イベントリレーション記録** (`db/events.rs`, `schema.sql`):
  - `event_relations` テーブルを新設。`events.send()` で `m.relates_to` が含まれる場合に INSERT IGNORE。
  - rel_type / relates_to_event_id を保存し、`GET /relations` の応答源として使用。

- **`GET /relations`** (`server/api/client/relations.rs`, `db/relations.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/relations/{eventId}[/{relType}[/{eventType}]]` の 3 バリアント。
  - stream_ordering ASC で並べ、`?from=<event_id>&limit=<n>` によるカーソルページネーション。
  - `chunk` / `next_batch` / `prev_batch` を返す。

- **`POST /read_markers`** (`server/api/client/read_markers.rs`):
  - `m.read` / `m.read.private` / `m.fully_read` を 1 リクエストで設定。
  - `m.read` 送信時は通知既読（`notifications.mark_read_up_to`）+ ハイライトクリア（`unread_highlights`）も同時実行。
  - `m.fully_read` は room account_data に保存。

## v0.29.0 でやったこと

- **`GET /notifications`** (`server/api/client/notifications.rs`, `db/notifications.rs`):
  - プッシュ通知履歴を返す新エンドポイント。
  - `notifications` テーブル新設（`schema.sql`）。`dispatch_push` が notify アクション発火時に INSERT。
  - `?from=<id>&limit=<n>` で ID ベースページネーション、`?only=highlight` でハイライトのみフィルタ。
  - `only=highlight` 時は `unread_highlights` テーブルと突き合わせて判定。

- **`/search` のページネーション** (`server/api/client/search.rs`, `db/events.rs`):
  - `?next_batch=<stream_ordering>` クエリパラメータを追加。
  - `search_room_events()` に `before_ordering: Option<i64>` を追加し、カーソル方式のページネーションを実装。

- **receipt POST 時の cleanup** (`server/api/client/receipts.rs`, `db/unread.rs`):
  - `m.read` / `m.read.private` 送信時に `notifications.mark_read_up_to()` で通知を既読にする。
  - 同時に `unread::delete_highlights_up_to()` でハイライトレコードを削除。

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
| /relations のページネーション | prev_batch は from トークンをそのまま返すのみ（後方ページングは未実装） |

## v0.33.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/rooms/{roomId}/timestamp_to_event`** — タイムスタンプから最近傍イベントを検索（MSC3030）
3. **`/account/3pid`** — メールアドレス等のサードパーティ ID 管理
4. **`m.room.canonical_alias` 連動** — `/aliases` 応答に `m.room.canonical_alias` 状態イベントの値も含める
5. **スレッドの latest_event 拡充** — `unsigned.m.relations.m.thread.latest_event` に完全なイベントオブジェクトを含める

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
