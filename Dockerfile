# ─── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /app

# Install only the system deps needed for native-tls
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests first (cache layer)
COPY Cargo.toml ./
# Create a dummy main so cargo can fetch deps without the real source
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release 2>&1 || true
RUN rm -rf src

# Now copy real source and rebuild (deps already cached)
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ─── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/hl-orderbook /app/hl-orderbook

# Default environment (override via docker-compose or Cloud Run)
ENV HL_TESTNET=false
ENV HL_COINS=BTC,ETH,SOL
ENV HL_MAX_DEPTH=20
ENV HL_ORDERBOOK_PORT=9090
ENV RUST_LOG=info

EXPOSE 9090

ENTRYPOINT ["/app/hl-orderbook"]
