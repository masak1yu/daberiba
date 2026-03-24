# Handover — v0.21.0 → v0.22.0

## v0.21.0 でやったこと

- **depth / prev_events 追跡** (`db::events` 拡張 + schema 変更):
  - `events` テーブルに `depth BIGINT NOT NULL DEFAULT 0` カラムを追加。
  - `db::events::get_room_tip()` 新設: ルームの最新イベントの depth と event_id を返す（次イベントの depth = max + 1、prev_events = [最新 event_id]）。
  - `db::events::send()` が内部で `get_room_tip()` を呼び、depth / prev_events を自動計算して保存する。戻り値を `(depth, prev_event_ids)` に変更。
  - `db::events::PduMeta` に `depth: i64` フィールドを追加。federation 受信 PDU も depth を保存。
  - `send_event` / `send_state_event` / `send_state_event_with_key` の各ハンドラが `get_room_tip()` を使って depth / prev_events を含む正確な PDU を組み立てるようになった。

- **`make_join` テンプレートの auth_events / depth / prev_events 設定**:
  - `db::room_state::get_auth_event_ids()` 新設: m.room.create / join_rules / power_levels の event_id を返す。
  - `make_join` がテンプレートに `auth_events`・`depth`・`prev_events` を正しく含めるようになった。

- **`preset: "public_chat"` 対応** (`rooms.rs create_room`):
  - `createRoom` リクエストで `preset: "public_chat"` を指定すると、`m.room.join_rules` の `join_rule` が `"invite"` ではなく `"public"` になる。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決が浅い | auth_events DAG の完全なグラフトラバーサルは未実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性（シングルスレッド的な運用では許容範囲） |
| auth_events が空のまま | 送信 PDU の auth_events=[]（make_join テンプレートは auth_event_ids を含むが、送信側 PDU 自体はまだ空） |

## v0.22.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **送信 PDU の auth_events 設定** — ローカルイベント送信時に正しい auth_events を含める
3. **federation send_join の room_version 伝播** — ローカルルームの room_version を send_join レスポンスで正確に返す（現在は rooms テーブルの値を返しているが、m.room.create の content と一致しているかを確認）
4. **`/sync` の state delta 最適化** — since 指定時に変更のあった state イベントのみ返す

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
