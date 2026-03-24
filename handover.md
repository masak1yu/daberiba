# Handover — v0.20.0 → v0.21.0

## v0.20.0 でやったこと

- **ルーム作成時の初期状態イベント自動生成** (`rooms.rs create_room` 拡張):
  - `POST /createRoom` でルームを作成する際に、以下の状態イベントを events / room_state テーブルへ保存するようにした。
    - `m.room.create` — creator + room_version: "10"
    - `m.room.join_rules` — join_rule: "invite"
    - `m.room.power_levels` — creator = 100、その他デフォルト値
    - `m.room.member` — creator の join
    - `m.room.name` / `m.room.topic`（リクエストで指定された場合のみ）
  - これにより federation `send_join` レスポンスの `auth_chain` が正しく返されるようになった。

- **ローカルルーム参加/退出時の federation 配送** (`rooms.rs join_room` / `leave_room` 拡張):
  - ローカルルームへの `join` / `leave` 時に `m.room.member` イベントを events テーブルへ保存し、`dispatch_send_transaction` で外部サーバーへ配送するようにした。
  - `leave_room` は leave イベントを保存してから `db::rooms::leave()` でメンバーシップを更新する順序に変更した。

- **`store_state_event` ヘルパー** (`rooms.rs` 内):
  - SHA-256 ハッシュで event_id を計算し `db::events::send()` を呼ぶ共通ヘルパーを追加。create_room の各状態イベント保存で再利用。

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
| send_transaction PDU の depth が 0 固定 | 送信 PDU の depth は常に 0。strict な受信側で問題になる可能性がある |
| auth_events / prev_events が空 | 送信 PDU の auth_events=[]、prev_events=[]。仕様上不正確 |
| join_room の m.room.member 二重書き込みリスク | join PDU 保存後に dispatch_send_transaction を呼ぶが、保存に失敗しても join 自体は完了する（ベストエフォート） |

## v0.21.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **Federation send_transaction の depth / prev_events 追跡** — ルームごとに最新イベントの depth を追跡し、送信 PDU に正確な depth・prev_events を付与する
3. **`make_join` テンプレートの auth_events 設定** — make_join が返すテンプレートに auth_events を正しく含める
4. **`publicRooms` join_rule public 対応** — createRoom で `preset: "public_chat"` を指定した場合に join_rule を public に変更する

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
