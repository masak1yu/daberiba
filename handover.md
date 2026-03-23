# Handover — v0.13.0 → v0.14.0

## v0.13.0 でやったこと

- **Federation 署名鍵の永続化**: Ed25519 鍵ペアを DB (`server_signing_key` テーブル) に保存。再起動後も同じ鍵を使用。DB 障害時はエフェメラル鍵にフォールバック（`SigningKey::load_or_generate`）。
- **X-Matrix 認証検証**: 受信 federation リクエストの署名を検証。他サーバーの公開鍵を `/_matrix/key/v2/server/<server_name>` で取得し `DashMap` にキャッシュ。`crates/server/src/xmatrix.rs` として独立モジュール化。`verify_request()` ヘルパーで各ハンドラから 1 行呼び出し可能。
- **Federation make_join / send_join**: `GET /_matrix/federation/v1/make_join/:room_id/:user_id` で join event テンプレート返却。`PUT /_matrix/federation/v2/send_join/:room_id/:event_id` で PDU を受け取り DB に格納、現在のルームステート + servers_in_room を返却。
- **Federation send_transaction**: `PUT /_matrix/federation/v1/send/:txn_id` で PDU を受信。`room_cache: HashMap<String, bool>` で N+1 クエリを回避。`m.room.member` 受信時にメンバーシップを更新。
- **sync timeline limited / prev_batch**: LIMIT 51 で 51 件取得し 50 件超えなら `limited: true`、`prev_batch` を最古イベントの `stream_ordering` トークンで設定。`ordering_to_token()` を `pub` に変更。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | receipts / room_aliases / presence / unread / room_tags / filters / to_device / keys / account_data / room_keys / server_signing_key は `sqlx::query()` 非マクロを使用 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| Federation 状態解決未実装 | send_join でルームステートをそのまま返却（state resolution アルゴリズム v2 未実装） |
| Federation PDU 署名検証が浅い | send_transaction / send_join で PDU の content 署名を検証していない（送信元サーバー認証のみ） |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |

## v0.14.0 候補

1. **Federation 状態解決** — Matrix state resolution algorithm v2 を実装（`send_join` 受信時に正しい auth_chain / state を計算）
2. **PDU 署名検証** — `send_transaction` / `send_join` 受信時に PDU 自体の Ed25519 署名を検証
3. **Federation `/event` エンドポイント** — `GET /_matrix/federation/v1/event/:event_id` でイベント取得（バックフィル対応）
4. **Room Version 対応** — room_version フィールドを DB に保持し、v1〜v10 のどのルームでも動作するよう対応
5. **Sync フィルタ適用強化** — `room.timeline.limit` / `room.state.limit` をフィルタ JSON から読んで適用

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
