# Handover — v0.18.0 → v0.19.0

## v0.18.0 でやったこと

- **Federation 送信 send_transaction** (`federation_client.rs` に追加):
  - ローカルユーザーがイベントを送信した際、同じルームに join している外部サーバーへ `PUT /_matrix/federation/v1/send/{txnId}` でイベントを配送するようにした。
  - `db::rooms::remote_servers_in_room()` 追加: ルーム内で join 中の外部サーバー名一覧を取得（`SUBSTRING_INDEX` で user_id からサーバー名を抽出）。
  - `federation_client::dispatch_send_transaction()` 追加: 背景 tokio タスクとしてベストエフォートで配送（失敗は warning ログのみ）。
  - `federation_client::sign_pdu()` 追加: 送信前に PDU へ自サーバーの Ed25519 署名を付与。
  - `events.rs` の `send_event` / `send_state_event` / `send_state_event_with_key` の各ハンドラから `dispatch_send_transaction` を呼び出し。
  - dispatch_push（HTTP pusher）と同様の非同期パターン。

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
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるようになったが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| PDU event_id が UUID ベース | ローカル生成イベントの event_id が `$uuid:server` 形式（room v1/v2 相当）で、room v3+ のハッシュベース形式ではない |
| send_transaction PDU に depth/auth_events/prev_events がない | 送信 PDU の depth=0、auth_events=[] は仕様上不正確。strict な受信側で拒否される可能性がある |

## v0.19.0 候補

1. **Federation leave 送信側** — ローカルユーザーが外部ルームを退出する際に make_leave → send_leave を発行する
2. **PDU event_id をハッシュベース (room v3+) に移行** — `db::events::send()` で UUID 形式をやめ SHA-256 ハッシュベースの event_id を生成する
3. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
4. **Federation send_transaction の depth / prev_events 追跡** — 送信 PDU に正確な depth・prev_events を付与する

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
