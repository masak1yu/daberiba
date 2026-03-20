set dotenv-load

# 開発環境起動（DB起動 + スキーマ自動適用）
up:
    podman compose up -d db migrate

# サーバーも含めて全起動（コンテナビルド済みの場合）
up-all:
    podman compose up -d

# 開発環境停止
down:
    podman compose down

# ログ確認
logs service="":
    podman compose logs -f {{ service }}

# ホスト上でサーバーを直接起動（開発用）
dev:
    cargo run --bin server

# ビルド
build:
    cargo build

# テスト
test:
    cargo test

# スキーマ適用（コンテナ内 mysqldef を使用）
schema-apply:
    podman compose run --rm tools just -f schema/justfile apply

# スキーマ dry-run（コンテナ内 mysqldef を使用）
schema-dry-run:
    podman compose run --rm tools just -f schema/justfile dry-run

# ツールイメージのビルド
build-tools:
    podman compose build tools

# サーバーイメージのビルド
build-server:
    podman compose build server

# ツールコンテナに入る
shell:
    podman compose exec tools bash
