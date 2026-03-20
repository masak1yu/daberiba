FROM docker.io/library/rust:1.78-slim-bookworm AS builder

WORKDIR /app

# 依存関係のキャッシュ層
COPY Cargo.toml Cargo.lock ./
COPY crates/server/Cargo.toml crates/server/
COPY crates/db/Cargo.toml crates/db/

# ダミーソースでキャッシュ
RUN mkdir -p crates/server/src crates/db/src \
    && echo "fn main(){}" > crates/server/src/main.rs \
    && echo "" > crates/db/src/lib.rs \
    && cargo build --release \
    && rm -rf crates/server/src crates/db/src

# 本番ビルド
COPY crates/ crates/
RUN touch crates/server/src/main.rs crates/db/src/lib.rs \
    && cargo build --release

FROM docker.io/library/debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/server /usr/local/bin/server

EXPOSE 8448

CMD ["server"]
