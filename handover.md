# Handover — v0.24.0 → v0.25.0

## v0.24.0 でやったこと

- **`createRoom` の `room_alias_name` 対応** (`rooms.rs`):
  - `CreateRoomRequest` に `room_alias_name: Option<String>` を追加。
  - 指定された場合、`#<alias_name>:<server_name>` 形式のエイリアスを `room_aliases` テーブルに登録し、`m.room.canonical_alias` 状態イベントを保存する。

- **`/rooms/{roomId}/context/{eventId}` エンドポイント** (`events.rs` + `db::events`):
  - `GET /_matrix/client/v3/rooms/{roomId}/context/{eventId}?limit=10` を新規実装。
  - 指定イベントの前後 `limit/2` 件ずつのイベントを返す（`events_before` / `events_after`）。
  - `start` / `end` トークン（`s{stream_ordering}` 形式）付き。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決が浅い | auth_events DAG の完全なグラフトラバーサルは未実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性（シングルスレッド的な運用では許容範囲） |
| /context の state フィールド | 現在は空配列を返している（指定時点のルームスナップショットは未実装） |

## v0.25.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/rooms/{roomId}/event/{eventId}` エンドポイント** — 単一イベント取得
3. **`/rooms/{roomId}/initialSync` または `/sync` の rooms.leave 対応** — 退出ルームの差分を返す
4. **push rule の `set_tweak: highlight` による highlight_count カウント** — 現在の LIKE 検索から push rule 評価ベースに移行

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
