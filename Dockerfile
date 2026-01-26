# Build stage
FROM rust:1.75-bookworm AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest files
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release && rm -rf src target/release/ergo-index*

# Copy actual source
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/ergo-index /usr/local/bin/ergo-index

# Copy UI files
COPY ui/static ./ui/static

# Create data directory
RUN mkdir -p /app/data

# Environment variables with defaults
ENV ERGO_NODES=http://host.docker.internal:9053
ENV DATABASE_PATH=/app/data/ergo-index.duckdb
ENV PORT=8080
ENV HOST=0.0.0.0
ENV SYNC_BATCH_SIZE=100
ENV SYNC_INTERVAL=10
ENV NETWORK=mainnet

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run
CMD ["ergo-index"]
