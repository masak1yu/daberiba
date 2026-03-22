FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Node.js LTS
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get install -y nodejs

# just (タスクランナー)
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh \
    | bash -s -- --to /usr/local/bin

# zsh テーマ: ブランチ表示（フォント不要）
RUN sed -i 's/ZSH_THEME="[^"]*"/ZSH_THEME="robbyrussell"/' /home/vscode/.zshrc

# mariadb-def (sqldef)
RUN ARCH="$(dpkg --print-architecture)" \
    && curl -fsSL "https://github.com/sqldef/sqldef/releases/latest/download/mariadb-def_linux_${ARCH}.tar.gz" \
    | tar xz -C /usr/local/bin mariadb-def \
    && chmod +x /usr/local/bin/mariadb-def
