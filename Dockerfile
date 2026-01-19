# ==============================================================================
# Stage 1: Chef base - install cargo-chef
# ==============================================================================
FROM rust:1.85-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ==============================================================================
# Stage 2: Planner - analyze dependencies
# ==============================================================================
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
RUN cargo chef prepare --recipe-path recipe.json

# ==============================================================================
# Stage 3: Builder - cache dependencies, then build
# ==============================================================================
FROM chef AS builder

# Cook dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY templates ./templates
COPY migrations ./migrations
RUN cargo build --release --bin shymini

# ==============================================================================
# Stage 4: Runtime - minimal image
# ==============================================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false shymini

WORKDIR /app

# Copy binary only (templates are compiled in, migrations embedded)
COPY --from=builder /app/target/release/shymini /app/shymini

# Create data directory
RUN mkdir -p /data && chown shymini:shymini /data

# Run as non-root user
USER shymini

ENV SHYMINI__HOST=0.0.0.0
ENV SHYMINI__PORT=8080
ENV SHYMINI__DATABASE_PATH=/data/shymini.db

EXPOSE 8080

CMD ["/app/shymini"]
