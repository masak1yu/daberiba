set dotenv-load

# 開発環境起動
up:
    podman compose up -d

# 開発環境停止
down:
    podman compose down

# ログ確認
logs:
    podman compose logs -f

# ビルド
build:
    cargo build

# 開発サーバ起動（ホスト上で直接）
dev:
    cargo run --bin server

# テスト
test:
    cargo test

# スキーマ適用
schema-apply:
    just -f schema/justfile apply

# スキーマ dry-run
schema-dry-run:
    just -f schema/justfile dry-run

# コンテナ内でサーバビルド
container-build:
    podman compose build

# コンテナ再起動
restart:
    podman compose restart server
