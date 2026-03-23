FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Bun (JavaScript runtime)
RUN curl -fsSL https://bun.sh/install | BUN_INSTALL=/usr/local bash

# just (タスクランナー)
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh \
    | bash -s -- --to /usr/local/bin

# zsh テーマ: ブランチ表示（フォント不要）
RUN sed -i 's/ZSH_THEME="[^"]*"/ZSH_THEME="robbyrussell"/' /home/vscode/.zshrc

