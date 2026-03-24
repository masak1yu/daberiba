# Handover — v0.25.0 → v0.26.0

## v0.25.0 でやったこと

- **`GET /rooms/{roomId}/event/{eventId}` エンドポイント** (`events.rs`):
  - 既存の `db::events::get_by_id()` を利用して単一イベントを返す。
  - イベントが存在しない場合は 404。

- **`/sync` の `rooms.leave` 対応** (`db::sync`, `db::rooms`):
  - `db::rooms::leave_rooms_since()` 新設: since_ordering より後に leave になったルームを返す。
  - 増分 sync 時に `rooms.leave` を正しく埋めるようになった（leave イベントを timeline に含む）。
  - 初回 sync（since なし）は従来通り空。

- **`highlight_count` の改善** (`db::unread`):
  - content 全体への LIKE 検索から `JSON_EXTRACT(content, '$.body')` + `JSON_EXTRACT(content, '$.formatted_body')` への LIKE 検索に変更。
  - ユーザーの localpart で検索することで `@alice:server` 形式のメンションに対応。
  - content 全体への誤ヒット（room_id などが content に含まれる場合）を防止。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は localpart LIKE | display name メンションや push rule の highlight tweak には未対応 |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決が浅い | auth_events DAG の完全なグラフトラバーサルは未実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性（シングルスレッド的な運用では許容範囲） |
| /context の state フィールド | 現在は空配列を返している（指定時点のルームスナップショットは未実装） |

## v0.26.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/rooms/{roomId}/redact/{eventId}/{txnId}` エンドポイント** — イベント削除（redaction）
3. **`/rooms/{roomId}/upgrade` エンドポイント** — room version アップグレード
4. **push rule の `highlight` tweak による正確な highlight_count** — `dispatch_push` でのハイライト評価結果を `unread_highlights` テーブルに記録

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
