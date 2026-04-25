# Handover — v1.0.0

## v1.0.0 でやったこと

v1.0.0 はバックエンドの機能追加に加え、フロントエンドクライアントの初回リリースを含む統合リリース。
以下のブランチを main にマージして v1.0.0 に統合した。

- `feature/v0.52.0` — receipt 差分配信、join_rules 検証、encryption フラグ、state イベント long-poll 起床
- `fix/db-env-cleanup` — DB 環境設定のクリーンアップ
- `fix/frontend-react19` — React 19 対応（フロントエンドの依存関係アップグレード）
- `frontend` — フロントエンドクライアント初期実装（ルーム一覧、タイムライン、メッセージ送信）
- `fix/element-design` — Element 風 UI 改善 + 2 件のクリティカルバグ修正

### クリティカルバグ修正（fix/element-design）

**`sync.rs` next_batch フォーマット欠落**（`crates/server/src/api/client/sync.rs`）:
- `format!()` に変数引数が渡されておらず、`next_batch` が literal `:stream_ordering_:max_to_device_id_:now_ms_:current_typing_version` という文字列として返却されていた。
- 結果として `parse_since` が不正なトークンを受け取り、long-polling が有効にならず sync がタイトループで暴走していた。
- 修正: `format!("{}_{}_{}_{}",stream_ordering, max_to_device_id, now_ms, current_typing_version)` に変更。

**`rooms.ts` state_key null フィルタ**（`front/src/stores/rooms.ts`）:
- サーバーは `m.room.message` などの非 state イベントに `"state_key": null` を返す。
- フロントエンドのフィルタが `e.state_key !== undefined` （厳密等価）を使用していたため `null !== undefined = true` となり、すべてのメッセージがタイムラインから除外されていた。
- 修正: `e.state_key != null` （ゆるい等価）に変更。

**同様の `:param` テンプレートバグを他ファイルでも修正**:
- `keys.rs:230` — `format!(":target_user_id/:key_id")` → `format!("{target_user_id}/{key_id}")`
- `rooms.rs:241,302` — `format!("federation join/leave failed: :e")` → `format!("... {e}")`
- `threads.rs:67-89` — SQL テンプレートの `:cursor_clause` / `:participated_clause` を `{}` に変更

### フロントエンド UI 改善（fix/element-design）

- **コンポーザー**: Element 風のメッセージ入力エリア。入力があるときのみ送信ボタンを表示。送信失敗時のエラートーストを追加。
- **Sidebar**: ホバーで ⋯ メニューを表示、アバター 32px、ハイライト背景 `#343a46`（Element のダークテーマカラー）。
- **DateSeparator**: タイムラインで日付が変わる境目に「今日」「昨日」「yyyy年M月d日」の区切りを挿入。
- **`userColor` ユーティリティ分離**: `Avatar.tsx` から `front/src/utils/userColor.ts` に移動（ESLint `react-refresh/only-export-components` 対応）。
- **MatrixEvent 型修正**: `state_key?: string | null`（旧: `string`）に修正。

## 次バージョン以降の候補

特定のロードマップは設定せず、実際に動かしながら出てきた問題を都度対処する方針。

潜在的な候補（優先度低）:
- SSO マルチプロバイダー対応（現状はシングルプロバイダーのみ、Google/Apple/GitHub は要リファクタリング）
- `rooms.encrypted` フラグを `/publicRooms` や `GET /rooms/{roomId}/summary` に活用
- `restricted` join_rule のフル実装
- receipt の `/sync` で `m.read.private` を非公開配信
- フロントエンド: スレッド表示、リアクション送信 UI、設定画面

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events DAG を使った完全な conflict resolution は未実装 |
| 3pid バリデーションなし | identity server 連携なし。登録は直接 INSERT のみ（メール確認なし） |
| /hierarchy の cross-server 展開 | federation ルームの子は room_state に m.space.child がない場合スキップされる |

## 開発フロー

```sh
# 環境起動（DB）
podman compose up -d db

# スキーマ適用
./dev schema-apply

# サーバー起動（バックエンド）
cargo run --bin server

# フロントエンド起動
cd front && pnpm dev

# テスト
cargo test
cargo clippy --all-targets -- -D warnings

# フォーマット
cargo fmt
```

## 環境設定

- `.env.example` → `.env` にコピーしてパスワードを設定
- `SQLX_OFFLINE=true` で DB なしビルド可能（`.sqlx/` がコミット済みのため CI でも動作）
- `MEDIA_BACKEND=s3` + `S3_BUCKET` で S3 に切り替え可能（`--features server/s3`）

## ブランチ戦略

- `main` — リリース済みタグのみマージ
- 作業は機能単位でブランチを切り、完成後に main へマージ
