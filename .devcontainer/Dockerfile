FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Node.js LTS
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get install -y nodejs

# just (タスクランナー)
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh \
    | bash -s -- --to /usr/local/bin

# zsh テーマ: ブランチ表示（フォント不要）
RUN sed -i 's/ZSH_THEME="[^"]*"/ZSH_THEME="robbyrussell"/' /home/vscode/.zshrc

# mariadb-def (sqldef) — dpkg の amd64 を x86_64 に変換
RUN DPKG_ARCH="$(dpkg --print-architecture)" \
    && case "$DPKG_ARCH" in \
        amd64) ARCH="x86_64" ;; \
        arm64) ARCH="arm64" ;; \
        *) ARCH="$DPKG_ARCH" ;; \
    esac \
    && curl -fsSL "https://github.com/sqldef/sqldef/releases/latest/download/mariadb-def_linux_${ARCH}.tar.gz" \
    | tar xz -C /usr/local/bin mariadb-def \
    && chmod +x /usr/local/bin/mariadb-def
