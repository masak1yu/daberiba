# Handover — v0.11.0 → v0.12.0

## v0.11.0 でやったこと

- **push_rules**: Matrix 仕様のデフォルトルールセット（override 7件、content 1件、underride 5件）をハードコード。ユーザー定義ルールは `account_data` の `m.push_rules` エントリとして保存。`GET /pushrules/` でデフォルト+カスタムを返す。`GET/PUT/DELETE /pushrules/{scope}/{kind}/{ruleId}` でルール操作。`/enabled` と `/actions` サブリソースも実装。新規 DB テーブルなし（account_data を流用）。
- **account_data sync since 対応**: `next_batch` トークンを `{stream_ordering}_{max_to_device_id}_{now_ms}` 形式に拡張。`since` がある場合は `account_data` の `updated_at > FROM_UNIXTIME(since_ms/1000.0)` で差分のみ返すように最適化。初回 sync（since なし）は全件返す。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | receipts / room_aliases / presence / unread / room_tags / filters / to_device / keys / account_data は `sqlx::query()` 非マクロを使用 |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| push_rules はサーバーサイド評価なし | プッシュルールの照合は未実装（ルール CRUD のみ） |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |

## v0.12.0 候補

1. **Federation 基盤** — `/_matrix/federation/` の基本実装（サーバー間通信）
2. **E2EE 鍵バックアップ** — `/_matrix/client/v3/room_keys/` エンドポイント（Megolm セッションキーのサーバー保管）
3. **push_rules サーバーサイド評価** — イベント送信時にルールを照合してプッシュ通知をフィルタ
4. **sync タイムライン限定配信** — `since` がある場合に joined rooms の state も差分のみ返す

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
