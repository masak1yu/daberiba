#!/bin/bash
set -e

# podman -> docker ブリッジ（Codespaces は Docker 環境）
sudo ln -sf "$(which docker)" /usr/local/bin/podman

# bun -> node の旧シムリンクを削除（残っている場合）
sudo unlink /usr/local/bin/node 2>/dev/null || true

# Claude Code のインストール（公式ネイティブバイナリ、Node.js 不要）
curl -fsSL https://claude.ai/install.sh | bash

# mysqldef (sqldef) のインストール — MariaDB 互換
DPKG_ARCH="$(dpkg --print-architecture)"
curl -fsSL "https://github.com/sqldef/sqldef/releases/latest/download/mysqldef_linux_${DPKG_ARCH}.tar.gz" \
    | sudo tar xz -C /usr/local/bin mysqldef
sudo chmod +x /usr/local/bin/mysqldef

# .env が未作成の場合、example から生成（開発用デフォルトパスワードで埋める）
if [ ! -f .env ]; then
    cp .env.example .env
    sed -i 's/<DB_PASS>/devpassword/g' .env
    sed -i 's/<your-password>/devpassword/g' .env
    sed -i 's/<your-root-password>/devrootpassword/g' .env
    echo ""
    echo "INFO: .env を開発用デフォルト値で作成しました。必要に応じて編集してください。"
fi
