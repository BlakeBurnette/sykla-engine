FROM debian:bookworm AS builder

# Install Rust via rustup (latest stable) + system deps
RUN apt-get update && apt-get install -y curl build-essential pkg-config libssl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Add WASM target and install wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown && \
    cargo install wasm-bindgen-cli

WORKDIR /app

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/sykla-core/Cargo.toml crates/sykla-core/Cargo.toml
COPY crates/sykla-server/Cargo.toml crates/sykla-server/Cargo.toml
COPY crates/sykla-world/Cargo.toml crates/sykla-world/Cargo.toml
COPY crates/sykla-web/Cargo.toml crates/sykla-web/Cargo.toml
COPY crates/sykla-engine/Cargo.toml crates/sykla-engine/Cargo.toml
COPY crates/sykla-mobile-core/Cargo.toml crates/sykla-mobile-core/Cargo.toml
COPY crates/sykla-city-gen/Cargo.toml crates/sykla-city-gen/Cargo.toml
COPY crates/sykla-godot/Cargo.toml crates/sykla-godot/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p crates/sykla-core/src && echo "// dummy" > crates/sykla-core/src/lib.rs && \
    mkdir -p crates/sykla-server/src && echo "fn main() {}" > crates/sykla-server/src/main.rs && \
    mkdir -p crates/sykla-world/src && echo "// dummy" > crates/sykla-world/src/lib.rs && \
    mkdir -p crates/sykla-web/src && echo "// dummy" > crates/sykla-web/src/lib.rs && \
    mkdir -p crates/sykla-engine/src && echo "// dummy" > crates/sykla-engine/src/lib.rs && \
    mkdir -p crates/sykla-mobile-core/src && echo "// dummy" > crates/sykla-mobile-core/src/lib.rs && \
    mkdir -p crates/sykla-city-gen/src && echo "fn main() {}" > crates/sykla-city-gen/src/main.rs && \
    mkdir -p crates/sykla-godot/src && echo "// dummy" > crates/sykla-godot/src/lib.rs && \
    mkdir -p migrations && touch migrations/.keep

# Build dependencies only (cached layer)
RUN cargo build --release -p sykla-server 2>/dev/null || true

# Copy real source code
COPY crates/ crates/
COPY migrations/ migrations/

# Force cargo to detect source changes (touch + remove old artifacts)
RUN rm -f target/release/sykla-server target/release/deps/sykla_server-* && \
    touch crates/sykla-server/src/main.rs crates/sykla-core/src/lib.rs crates/sykla-world/src/lib.rs

# Build server
RUN cargo build --release -p sykla-server

# Build WASM frontend
RUN cargo build --target wasm32-unknown-unknown --release -p sykla-web && \
    wasm-bindgen --target web --out-dir web/dist \
        target/wasm32-unknown-unknown/release/sykla_web.wasm

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sykla-server /usr/local/bin/sykla-server
COPY --from=builder /app/migrations /app/migrations

# Copy web frontend (static HTML + WASM)
COPY web/ /app/web/
COPY --from=builder /app/web/dist /app/web/dist

WORKDIR /app
ENV WEB_DIR=/app/web
EXPOSE 8080

CMD ["sykla-server"]
