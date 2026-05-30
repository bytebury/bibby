FROM lukemathwalker/cargo-chef:latest-rust-1.95 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C target-feature=+crt-static"
ENV SQLX_OFFLINE=true

# Cache dependency builds via cargo-chef. This layer only invalidates when
# Cargo.toml / Cargo.lock change, so application code edits stay fast.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM node:22-alpine AS assets-processor
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install --no-audit --no-fund
COPY public ./public
COPY templates ./templates
COPY src ./src
RUN npx tailwindcss -i ./public/styles/tailwind.css -o ./public/styles.css --minify
RUN npm i -g terser clean-css-cli
RUN find public -name "*.js" -exec sh -c 'terser "$1" --compress --mangle -o "$1"' _ {} \; && \
    find public -name "*.css" -exec sh -c 'cleancss -O2 -o "$1" "$1"' _ {} \;

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/bibby ./app
COPY --from=builder /app/templates ./templates
COPY --from=assets-processor /app/public ./public

CMD ["./app"]
