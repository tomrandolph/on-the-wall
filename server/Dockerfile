# ---- build stage ----
# pin a toolchain new enough for edition 2024 (needs Rust >= 1.85)
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /app

# build against the committed offline query cache; no database needed at build time
ENV SQLX_OFFLINE=true

# 1) cache the dependency build: copy manifests, build a stub, so this layer
#    is only invalidated when Cargo.toml / Cargo.lock change
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# 2) build the real application
COPY . .
# bust the stub's cached artifact so the real main.rs is recompiled
RUN touch src/main.rs && cargo build --release

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime

# ca-certificates is needed to validate the Postgres server's TLS cert (rustls)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# run as a non-root user
RUN useradd --create-home --user-group app
USER app
WORKDIR /home/app

# migrations are embedded into the binary at compile time via sqlx::migrate!,
# so only the binary itself is needed at runtime
COPY --from=builder /app/target/release/server ./server

# informational; Render injects the real port via $PORT at runtime
EXPOSE 10000

CMD ["./server"]
