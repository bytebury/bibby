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
# Baked in by `option_env!("GIT_HASH")` and shown on /admin/monitoring.
# `.git` isn't in the build context, so pass it in:
#   docker build --build-arg GIT_HASH=$(git rev-parse --short HEAD) .
ARG GIT_HASH=""
ENV GIT_HASH=${GIT_HASH}
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM node:22-alpine AS assets-processor
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
RUN npm i -g terser@5.51.2 lightningcss-cli@1.33.0 --no-audit --no-fund
COPY public ./public
# Tailwind scans these for class names (see the @source lines in tailwind.css).
COPY templates ./templates
COPY src ./src
RUN npx tailwindcss -i ./public/styles/tailwind.css -o ./public/styles.css --minify
# Vendored `*.min.js` files are already minified; running terser over them
# again only risks breaking them.
RUN find public -name "*.js" ! -name "*.min.js" -exec sh -c 'terser "$1" --compress --mangle -o "$1"' _ {} \; && \
    find public -name "*.css" -exec sh -c 'lightningcss --minify --targets ">= 0.25%" -o "$1" "$1"' _ {} \;

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin app
WORKDIR /app

# Templates are compiled into the binary by askama, so only the binary and
# the static files served under /assets ship.
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/bibby ./app
COPY --from=assets-processor /app/public ./public

USER app
EXPOSE 8080
CMD ["./app"]
