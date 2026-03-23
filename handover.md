# Handover — v0.12.0 → v0.13.0

## v0.12.0 でやったこと

- **sync state 差分**: 初回 sync（since なし）で `room_state` JOIN `events` から現在のステートスナップショットを `state.events` に返すよう修正。増分 sync（since あり）は従来どおり空配列（ステート変更は timeline に含まれる）。
- **E2EE 鍵バックアップ** (`/_matrix/client/v3/room_keys/`): Megolm セッションキーのサーバー保管を実装。新規 DB テーブル `room_key_backup_versions` + `room_key_backup_sessions`。バージョン管理、全・ルーム・セッション単位の GET/PUT/DELETE をすべて実装。`first_message_index` が小さい方を優先する upsert ロジック付き。
- **push_rules サーバーサイド評価**: イベント送信時に受信者ごとの push rules を評価し、notify アクションがある場合のみ HTTP pusher へ配送。評価する条件種別: `event_match`（glob）、`contains_display_name`、`room_member_count`、`sender_notification_permission`（簡易許可）。ルームメンバー数取得に `db::rooms::count_joined_members` を追加。
- **Federation 基盤**: Ed25519 署名鍵を起動時生成（`ed25519-dalek` 2.x）。`GET /_matrix/key/v2/server` でサーバー公開鍵を返却（レスポンス自体を署名）。`GET /_matrix/federation/v1/version` でサーバーバージョン返却。`GET /_matrix/federation/v1/query/directory` でルームエイリアス解決（他サーバーからの問い合わせ対応）。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | receipts / room_aliases / presence / unread / room_tags / filters / to_device / keys / account_data / room_keys は `sqlx::query()` 非マクロを使用 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| Federation 署名鍵は起動時生成 | 再起動のたびに鍵が変わる。永続化（DB またはファイル）は未実装 |
| Federation X-Matrix 認証は未検証 | 受信リクエストの署名検証なし（構造チェックのみ） |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |

## v0.13.0 候補

1. **Federation 署名鍵の永続化** — DB またはファイルに Ed25519 鍵ペアを保存（再起動しても鍵が変わらない）
2. **X-Matrix 認証検証** — 受信 federation リクエストの署名を検証（他サーバーの公開鍵を `/_matrix/key/v2/server/<server_name>` で取得してキャッシュ）
3. **Federation send_join / make_join** — 他サーバーのルームへの参加フロー
4. **Federation send transaction** — `PUT /_matrix/federation/v1/send/{txnId}` でイベント受信
5. **sync timeline limited** — 大量イベントがある場合に `limited: true` と `prev_batch` を正しく設定

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
cargo test

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
