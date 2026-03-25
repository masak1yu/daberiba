# Handover — v0.29.0 → v0.30.0

## v0.29.0 でやったこと

- **`GET /notifications`** (`server/api/client/notifications.rs`, `db/notifications.rs`):
  - プッシュ通知履歴を返す新エンドポイント。
  - `notifications` テーブル新設（`schema.sql`）。`dispatch_push` が notify アクション発火時に INSERT。
  - `?from=<id>&limit=<n>` で ID ベースページネーション、`?only=highlight` でハイライトのみフィルタ。
  - `only=highlight` 時は `unread_highlights` テーブルと突き合わせて判定。

- **`/search` のページネーション** (`server/api/client/search.rs`, `db/events.rs`):
  - `?next_batch=<stream_ordering>` クエリパラメータを追加。
  - `search_room_events()` に `before_ordering: Option<i64>` を追加し、カーソル方式のページネーションを実装。
  - `limit+1` 件取得してページ継続を判定し、ある場合は `next_batch` フィールドに末尾の `stream_ordering` を返す。

- **receipt POST 時の cleanup** (`server/api/client/receipts.rs`, `db/unread.rs`):
  - `m.read` / `m.read.private` 送信時に `notifications.mark_read_up_to()` で通知を既読にする。
  - 同時に `unread::delete_highlights_up_to()` でハイライトレコードを削除。
  - `db::unread::is_highlight()` / `delete_highlights_up_to()` を新設。

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
| /notifications の only=highlight | unread_highlights テーブルを二次参照するため、削除済みでも結果が空になる可能性あり |

## v0.30.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/rooms/{roomId}/threads` エンドポイント** — MSC スレッド対応（m.thread rel_type）
3. **`/relations` エンドポイント** — イベントリレーション（編集・リアクション等）の取得
4. **イベント編集サポート** — `m.replace` rel_type に対応した `/event` / `/messages` の内容置き換え
5. **`/rooms/{roomId}/read_markers`** — バッチ既読マーカー（`m.read` + `m.fully_read` 一括送信）

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
