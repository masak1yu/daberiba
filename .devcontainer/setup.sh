#!/bin/bash
set -e

# podman -> docker ブリッジ（Codespaces は Docker 環境）
sudo ln -sf "$(which docker)" /usr/local/bin/podman

# Claude Code のインストール
npm install -g @anthropic-ai/claude-code

# .env が未作成の場合、example から生成（開発用デフォルトパスワードで埋める）
if [ ! -f .env ]; then
    cp .env.example .env
    sed -i 's/<DB_PASS>/devpassword/g' .env
    sed -i 's/<your-password>/devpassword/g' .env
    sed -i 's/<your-root-password>/devrootpassword/g' .env
    echo ""
    echo "INFO: .env を開発用デフォルト値で作成しました。必要に応じて編集してください。"
fi
