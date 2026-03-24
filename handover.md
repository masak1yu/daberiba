# Handover — v0.26.0 → v0.27.0

## v0.26.0 でやったこと

- **モデレーション系エンドポイント** (`rooms.rs`):
  - `POST /rooms/{roomId}/kick` — 対象ユーザーの m.room.member leave イベントを保存し、membership を 'leave' に更新。
  - `POST /rooms/{roomId}/ban` — 対象ユーザーの m.room.member ban イベントを保存し、membership を 'ban' に更新。
  - `POST /rooms/{roomId}/unban` — membership を 'ban' → 'leave' に戻す（再招待可能状態）。
  - `POST /rooms/{roomId}/forget` — leave/ban 状態のユーザーがルームの記録を削除。

- **Redaction エンドポイント** (`rooms.rs`, `db::events`):
  - `PUT /rooms/{roomId}/redact/{eventId}/{txnId}` — m.room.redaction メッセージイベントを保存し、対象イベントの content を `{}` に置換。
  - `db::events::redact_event()` 新設: `UPDATE events SET content = '{}'`。

- **`store_message_event` ヘルパー追加** (`rooms.rs`):
  - state_key なしのメッセージイベントを保存する共通ヘルパー（`store_state_event` の非 state_key 版）。
  - redaction に使用。

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
| kick/ban の権限チェックなし | 送信者の power_level を検証していない |
| redaction の権限チェックなし | 自分のイベントか管理者権限かの確認が未実装 |

## v0.27.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **`/rooms/{roomId}/upgrade` エンドポイント** — room version アップグレード
3. **push rule の `highlight` tweak による正確な highlight_count** — `dispatch_push` でのハイライト評価結果を `unread_highlights` テーブルに記録
4. **モデレーション権限チェック** — kick/ban/redact 時の power_level 検証

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
