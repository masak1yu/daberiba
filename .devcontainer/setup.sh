#!/bin/bash
set -e

# podman -> docker ブリッジ（Codespaces は Docker 環境）
sudo ln -sf "$(which docker)" /usr/local/bin/podman

# Claude Code のインストール（公式ネイティブバイナリ、Node.js 不要）
# ボリュームマウント時に root 所有になるため、先に vscode へ chown する
sudo mkdir -p ~/.claude/downloads
sudo chown -R "$(id -u):$(id -g)" ~/.claude
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
