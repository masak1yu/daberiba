# Handover — v0.23.0 → v0.24.0

## v0.23.0 でやったこと

- **`LocalEvent` リファクタリング** (`db::events`):
  - `LocalEvent` に `depth: i64` と `prev_events: &[String]` フィールドを追加。
  - `send()` 内部の `get_room_tip()` 二重呼び出しを廃止。呼び出し元が事前に取得した depth/prev_events をそのまま使うことで、event_id（PDU ハッシュから計算）と保存フィールドが一致するようになった。
  - 戻り値を `Result<()>` に変更（呼び出し元はすでに depth/prev_events を持っているため）。

- **`store_state_event` の auth_events/depth/prev_events 設定** (`rooms.rs`):
  - `tokio::join!` で `get_room_tip` + `get_auth_event_ids` を並列取得。
  - PDU に正しい depth・prev_events・auth_events を含めて event_id を計算するようになった。
  - 戻り値を `(String, serde_json::Value)` に変更（event_id と PDU）。
  - `join_room` と `leave_room` のインライン PDU 構築を廃止し、`store_state_event` の戻り値を federation 配送に使用。

- **presence 全メンバー対応** (`db::presence`):
  - `get_for_room_members` を INNER JOIN → LEFT JOIN に変更。
  - `PUT /presence` を一度も呼んでいないユーザーも "offline" をデフォルトとして sync の `presence.events` に出現するようになった。

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

## v0.24.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **Room alias 自動登録** — `createRoom` 時に `room_alias_name` を受け取って `m.room.aliases` 状態イベントを保存 + エイリアス登録
3. **`/rooms/{roomId}/members` エンドポイント** — ルームメンバーリスト取得
4. **`/rooms/{roomId}/context/{eventId}` エンドポイント** — イベントコンテキスト取得（メッセージスクロール対応）

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
