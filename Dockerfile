FROM debian:bookworm AS builder

# Install Rust via rustup (latest stable)
RUN apt-get update && apt-get install -y curl build-essential pkg-config libssl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/sykla-core/Cargo.toml crates/sykla-core/Cargo.toml
COPY crates/sykla-server/Cargo.toml crates/sykla-server/Cargo.toml
COPY crates/sykla-world/Cargo.toml crates/sykla-world/Cargo.toml
COPY crates/sykla-web/Cargo.toml crates/sykla-web/Cargo.toml
COPY crates/sykla-mobile-core/Cargo.toml crates/sykla-mobile-core/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p crates/sykla-core/src && echo "// dummy" > crates/sykla-core/src/lib.rs && \
    mkdir -p crates/sykla-server/src && echo "fn main() {}" > crates/sykla-server/src/main.rs && \
    mkdir -p crates/sykla-world/src && echo "// dummy" > crates/sykla-world/src/lib.rs && \
    mkdir -p crates/sykla-web/src && echo "// dummy" > crates/sykla-web/src/lib.rs && \
    mkdir -p crates/sykla-mobile-core/src && echo "// dummy" > crates/sykla-mobile-core/src/lib.rs && \
    mkdir -p migrations && touch migrations/.keep

# Build dependencies only (cached layer)
RUN cargo build --release -p sykla-server 2>/dev/null || true

# Copy real source code
COPY crates/ crates/
COPY migrations/ migrations/

# Force cargo to detect source changes (touch + remove old artifacts)
RUN rm -f target/release/sykla-server target/release/deps/sykla_server-* && \
    touch crates/sykla-server/src/main.rs crates/sykla-core/src/lib.rs crates/sykla-world/src/lib.rs

# Rebuild with real source
RUN cargo build --release -p sykla-server

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sykla-server /usr/local/bin/sykla-server
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app
EXPOSE 8080

CMD ["sykla-server"]
