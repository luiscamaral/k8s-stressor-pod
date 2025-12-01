# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

# Build stage
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/api && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

# Build application
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

LABEL org.opencontainers.image.source="https://github.com/luiscamaral/k8s-stressor-pod"
LABEL org.opencontainers.image.description="k8s-stressor - Kubernetes reliability testing"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.authors="Luis Amaral"
LABEL org.opencontainers.image.version="0.2.0"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -s /bin/false stressor

COPY --from=builder /app/target/release/k8s-stressor /usr/local/bin/

USER 1000

EXPOSE 8080

ENV RUST_LOG=info

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["k8s-stressor"]
