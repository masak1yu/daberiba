# Handover — v0.2.0 → v0.3.0

## v0.2.0 でやったこと

- **sync cursor 改善**: `events` テーブルに `stream_ordering BIGINT AUTO_INCREMENT` を追加。`since`/`next_batch` をタイムスタンプ依存から stream_ordering ベースに変更し、同一ミリ秒内の複数イベント取りこぼし問題を解消
- **デバイス管理 API**: `devices` テーブル新規追加。`GET/PUT/DELETE /devices/{deviceId}`, `POST /delete_devices` の5エンドポイントを実装。register/login 時に devices テーブルへ自動登録
- **パスワード変更 API**: `POST /account/password`（旧パスワード検証 → argon2 再ハッシュ → UPDATE）
- **メディア API**: `MediaStore` trait で Local/S3 差し替え可能な設計。`LocalStore` 実装（サブディレクトリ分散）。`POST /upload`, `GET /download/{serverName}/{mediaId}`
- **Dockerfile.tools アーキテクチャ対応**: `TARGETARCH` ハードコードを廃止し `uname -m` 自動検出に変更（Mac/WSL 両対応）
- **GitGuardian 対策**: `compose.yml` からパスワードのデフォルト値（`changeme`）を撤廃。`.env.example` をプレースホルダー形式に変更
- **sqlx offline mode**: 現在 `sqlx::query()`（runtime）を使用しているため DB なしで `cargo build` が通る。`SQLX_OFFLINE=true` も動作確認済み

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| sqlx compile-time check 未使用 | `query!()` マクロへ移行すると型安全になるが大規模リファクタ。移行後に `cargo sqlx prepare` → `.sqlx/` をコミット |
| パスワード変更の UIA 未実装 | `/account/password` は現在 Bearer 認証のみ。Matrix 仕様では User Interactive Authentication が推奨 |
| delete_devices の UIA 未実装 | 同上。`POST /delete_devices` も本来 UIA が必要 |
| メディアの S3 対応未実装 | `MediaStore` trait は用意済み。`S3Store` を追加するだけで差し替え可能 |
| メディアのアクセス制御なし | 現状は認証ユーザー全員がダウンロード可能 |
| devices の last_seen_ts/ip 未更新 | テーブルはあるが API リクエスト時に更新するロジックがない |
| dnsname CNI プラグイン問題（WSL） | Ubuntu 20.04 + podman 3.4.2 では dnsname が動かないため `podman compose up migrate` が失敗する。`mysql` クライアント直接接続で回避 |
| compose TLS workaround | GitHub からのバイナリ取得に `curl -k` を使用。社内 CA 証明書をコンテナに追加するのが正式対応 |

## v0.3.0 でやること（候補）

### 1. Push Notification（任意）
- `POST /_matrix/client/v3/pushers/set`

### 2. sqlx query! マクロ移行
全クエリを `query!()` / `query_as!()` に移行してコンパイル時型チェックを有効化
```sh
DATABASE_URL=... cargo sqlx prepare --workspace
git add .sqlx/
```

### 3. UIA（User Interactive Authentication）
- パスワード変更・デバイス削除に UIA フローを追加
- `POST /_matrix/client/v3/auth/...`

### 4. メディア S3 対応
```rust
// S3Store を実装して MEDIA_BACKEND=s3 で切り替え
pub struct S3Store { ... }
impl MediaStore for S3Store { ... }
```

### 5. devices last_seen 更新
認証ミドルウェアで `devices.last_seen_ts` / `last_seen_ip` を更新

## 開発フロー（おさらい）

```sh
# 環境起動（DB）
podman compose up -d db

# スキーマ適用（dnsname 問題がある場合は直接接続）
mysql -h 127.0.0.1 -P 13306 -u matrix -p matrix < schema/schema.sql

# ホストでサーバ起動
cargo run --bin server

# スキーマ変更時
#   1. schema/schema.sql を編集
#   2. dry-run で確認
./dev schema-dry-run
#   3. 適用
./dev schema-apply

# テスト
cargo test

# フォーマット
cargo fmt
```

## 環境設定

- `.env.example` → `.env` にコピーしてパスワードを設定（`.env` は gitignore 済み）
- `MEDIA_PATH` でメディア保存先を変更可能（デフォルト `./media`）
- `SQLX_OFFLINE=true` で DB なしビルド可能

## ブランチ戦略

- `main` — リリース済みタグのみマージ
- `feature/v0.x.0` — バージョン単位の作業ブランチ
- 機能単位でさらに feature ブランチを切っても良い
