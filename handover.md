# Handover — v0.17.0 → v0.18.0

## v0.17.0 でやったこと

- **Federation 送信側の実装** (`federation_client.rs` 新規追加):
  - `POST /_matrix/client/v3/join/{roomId}` で外部ルームを検出した場合、make_join → sign PDU → send_join フローを自動実行するようにした。
  - `is_local_room()`: room_id の server 部分が自サーバー名と一致するかどうかで内外を判定。
  - `join_remote_room()`: GET make_join でテンプレート取得 → Ed25519 署名付与 → event_id 計算（room version 3+, SHA-256 ハッシュ）→ PUT send_join 送信 → レスポンスの state/auth_chain PDU を DB に保存。
  - X-Matrix 送信ヘッダーは `xmatrix::make_auth_header()` で生成（既存の X-Matrix 検証コードと対称的な実装）。
  - `signing_key::compute_event_id()` 追加: signatures/unsigned/event_id/hashes を除いたカノニカル JSON の SHA-256 を URL-safe unpadded base64 エンコード。
  - `db::rooms::set_version()` 追加: send_join レスポンスの room_version を DB に保存。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| highlight_count は LIKE 検索 | content に user_id 文字列が含まれるかどうかの簡易実装 |
| プレゼンスは登録ユーザーのみ | `PUT /presence` を一度も呼んでいないユーザーは sync の presence.events に出現しない |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決が浅い | auth_events DAG の完全なグラフトラバーサルは未実装。`send_join` の auth_chain は m.room.create/join_rules/power_levels のみ |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるようになったが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| Federation 送信 send_transaction 未実装 | 自サーバーで発生したイベントを他サーバーへ PUT send_transaction で配送する機能がない |

## v0.18.0 候補

1. **Federation 送信 send_transaction** — ローカルイベント送信時に、同じルームの他サーバーメンバーへ PUT `/_matrix/federation/v1/send/{txnId}` でイベントを配送する
2. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
3. **Federation leave 送信側** — ローカルユーザーが外部ルームを退出する際に make_leave → send_leave を発行する
4. **notify_push_gateway (federation 経由)** — send_transaction で届いたイベントに対して、ローカルユーザーへの push 配送

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
