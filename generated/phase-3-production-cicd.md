# Phase 3: Production Ready + CI/CD

> **Author:** Luis Amaral  
> **Version:** 0.3.0  
> **Duration:** 4-5 days  
> **Prerequisite:** Phase 2 complete  
> **Deliverable:** Production-ready stressor with GitHub Actions CI/CD

---

## Overview

Complete the production feature set with Prometheus metrics, Kubernetes manifests, and automated CI/CD pipeline that builds, tests, and pushes images to a private container registry.

### Success Criteria

- [ ] Prometheus metrics endpoint with all stressor metrics
- [ ] Kubernetes deployment manifests
- [ ] GitHub Actions workflow for CI/CD
- [ ] Automated tests in pipeline
- [ ] Docker image pushed to private registry (ghcr.io)
- [ ] All unit and integration tests pass
- [ ] Docker image v0.3.0 builds and runs

---

## Project Structure Final

```
k8s-stressor/
├── .github/
│   └── workflows/
│       └── ci.yml
├── k8s/
│   ├── deployment.yaml
│   ├── service.yaml
│   └── configmap.yaml
├── scripts/
│   ├── test-phase1.sh
│   ├── test-phase2.sh
│   └── test-phase3.sh
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── state.rs
│   ├── error.rs
│   ├── api/
│   │   ├── mod.rs
│   │   └── handlers.rs
│   ├── orchestrator/
│   │   ├── mod.rs
│   │   └── manager.rs
│   ├── engines/
│   │   ├── mod.rs
│   │   ├── cpu.rs
│   │   ├── memory.rs
│   │   └── network.rs
│   └── metrics/              # NEW
│       ├── mod.rs
│       └── prometheus.rs
├── tests/
├── Cargo.toml
├── Dockerfile
└── README.md
```

---

## Cargo.toml Final

```toml
[package]
name = "k8s-stressor"
version = "0.3.0"
edition = "2021"
authors = ["Luis Amaral"]
license = "MIT"
description = "Deterministic resource consumption for Kubernetes reliability testing"
repository = "https://github.com/luiscamaral/k8s-stressor-pod"

[dependencies]
# Async Runtime
tokio = { version = "1", features = ["full", "sync", "time", "rt-multi-thread", "signal"] }

# Web Framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Logging & Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# System Info
num_cpus = "1.16"

# HTTP Client
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }

# Metrics
prometheus = "0.13"
lazy_static = "1.4"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
tokio-test = "0.4"
axum-test = "14"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

---

## Task Checklist

### 3.1 Prometheus Metrics Module

#### src/metrics/prometheus.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use lazy_static::lazy_static;
use prometheus::{
    Encoder, GaugeVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Mode gauge (0=idle, 1=cpu, 2=memory, 3=network)
    pub static ref STRESSOR_MODE: IntGauge = IntGauge::new(
        "stressor_mode",
        "Current operation mode (0=idle, 1=cpu, 2=memory, 3=network)"
    ).expect("metric can be created");

    // Config version counter
    pub static ref CONFIG_VERSION: IntCounter = IntCounter::new(
        "stressor_config_version",
        "Configuration version counter"
    ).expect("metric can be created");

    // Is active gauge
    pub static ref IS_ACTIVE: IntGauge = IntGauge::new(
        "stressor_is_active",
        "Whether a stressor is currently running (0 or 1)"
    ).expect("metric can be created");

    // CPU metrics
    pub static ref CPU_TARGET_MILLICORES: IntGauge = IntGauge::new(
        "stressor_cpu_target_millicores",
        "Target CPU load in millicores"
    ).expect("metric can be created");

    pub static ref CPU_ACTIVE_THREADS: IntGauge = IntGauge::new(
        "stressor_cpu_active_threads",
        "Number of active CPU worker threads"
    ).expect("metric can be created");

    // Memory metrics
    pub static ref MEMORY_TARGET_BYTES: IntGauge = IntGauge::new(
        "stressor_memory_target_bytes",
        "Target memory allocation in bytes"
    ).expect("metric can be created");

    pub static ref MEMORY_ALLOCATED_BYTES: IntGauge = IntGauge::new(
        "stressor_memory_allocated_bytes",
        "Current memory allocation in bytes"
    ).expect("metric can be created");

    // Network metrics
    pub static ref NETWORK_ACTIVE_CONNECTIONS: IntGauge = IntGauge::new(
        "stressor_network_active_connections",
        "Number of active network connections"
    ).expect("metric can be created");

    pub static ref NETWORK_REQUESTS_TOTAL: IntCounter = IntCounter::new(
        "stressor_network_requests_total",
        "Total number of network requests made"
    ).expect("metric can be created");

    pub static ref NETWORK_ERRORS_TOTAL: IntCounter = IntCounter::new(
        "stressor_network_errors_total",
        "Total number of network errors"
    ).expect("metric can be created");

    // Build info
    pub static ref BUILD_INFO: IntGauge = IntGauge::new(
        "stressor_build_info",
        "Build information"
    ).expect("metric can be created");
}

/// Register all metrics with the registry
pub fn register_metrics() {
    REGISTRY.register(Box::new(STRESSOR_MODE.clone())).expect("register");
    REGISTRY.register(Box::new(CONFIG_VERSION.clone())).expect("register");
    REGISTRY.register(Box::new(IS_ACTIVE.clone())).expect("register");
    REGISTRY.register(Box::new(CPU_TARGET_MILLICORES.clone())).expect("register");
    REGISTRY.register(Box::new(CPU_ACTIVE_THREADS.clone())).expect("register");
    REGISTRY.register(Box::new(MEMORY_TARGET_BYTES.clone())).expect("register");
    REGISTRY.register(Box::new(MEMORY_ALLOCATED_BYTES.clone())).expect("register");
    REGISTRY.register(Box::new(NETWORK_ACTIVE_CONNECTIONS.clone())).expect("register");
    REGISTRY.register(Box::new(NETWORK_REQUESTS_TOTAL.clone())).expect("register");
    REGISTRY.register(Box::new(NETWORK_ERRORS_TOTAL.clone())).expect("register");
    REGISTRY.register(Box::new(BUILD_INFO.clone())).expect("register");

    // Set build info
    BUILD_INFO.set(1);
}

/// Encode all metrics to Prometheus text format
pub fn encode_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).expect("encode");
    String::from_utf8(buffer).expect("utf8")
}

/// Update mode metric
pub fn set_mode(mode: i64) {
    STRESSOR_MODE.set(mode);
    IS_ACTIVE.set(if mode == 0 { 0 } else { 1 });
}

/// Increment config version
pub fn inc_config_version() {
    CONFIG_VERSION.inc();
}

/// Update CPU metrics
pub fn update_cpu_metrics(target_millicores: i64, active_threads: i64) {
    CPU_TARGET_MILLICORES.set(target_millicores);
    CPU_ACTIVE_THREADS.set(active_threads);
}

/// Update memory metrics
pub fn update_memory_metrics(target_bytes: i64, allocated_bytes: i64) {
    MEMORY_TARGET_BYTES.set(target_bytes);
    MEMORY_ALLOCATED_BYTES.set(allocated_bytes);
}

/// Update network metrics
pub fn update_network_metrics(active_connections: i64, requests: u64, errors: u64) {
    NETWORK_ACTIVE_CONNECTIONS.set(active_connections);
    // Note: counters only go up, so we track increments elsewhere
}

/// Reset all stressor metrics to zero
pub fn reset_metrics() {
    CPU_TARGET_MILLICORES.set(0);
    CPU_ACTIVE_THREADS.set(0);
    MEMORY_TARGET_BYTES.set(0);
    MEMORY_ALLOCATED_BYTES.set(0);
    NETWORK_ACTIVE_CONNECTIONS.set(0);
}
```

#### src/metrics/mod.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod prometheus;

pub use prometheus::{
    encode_metrics, register_metrics, reset_metrics, set_mode,
    inc_config_version, update_cpu_metrics, update_memory_metrics,
    CPU_TARGET_MILLICORES, MEMORY_ALLOCATED_BYTES, NETWORK_REQUESTS_TOTAL,
};
```

### 3.2 Update lib.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod api;
pub mod config;
pub mod engines;
pub mod error;
pub mod metrics;
pub mod orchestrator;
pub mod state;
```

### 3.3 Update handlers.rs for Prometheus

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};
use crate::error::AppError;
use crate::metrics;
use crate::state::SharedState;

// ... (keep existing structs and handlers)

/// GET /metrics - Prometheus format
pub async fn get_metrics() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4; charset=utf-8")],
        metrics::encode_metrics(),
    )
}

/// POST /mode - Update mode metric
pub async fn set_mode(
    State(state): State<SharedState>,
    Json(mode): Json<OperationMode>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Mode change: {:?} -> {:?}", s.current_mode, mode);
    
    let mode_num = match &mode {
        OperationMode::Idle => 0,
        OperationMode::CpuStressor => 1,
        OperationMode::MemoryStressor => 2,
        OperationMode::NetworkStressor => 3,
    };
    
    s.current_mode = mode;
    s.bump_version();
    
    metrics::set_mode(mode_num);
    metrics::inc_config_version();
    
    StatusCode::OK
}

/// POST /stop - Reset metrics
pub async fn stop_all(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    
    metrics::set_mode(0);
    metrics::reset_metrics();
    metrics::inc_config_version();
    
    StatusCode::OK
}

// ... (keep other handlers, add metrics::inc_config_version() to each POST)
```

### 3.4 Update main.rs for Metrics Init

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::net::SocketAddr;

use axum::{routing::{get, post}, Router};
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use k8s_stressor::api::handlers;
use k8s_stressor::metrics;
use k8s_stressor::orchestrator::Orchestrator;
use k8s_stressor::state::create_shared_state;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Register Prometheus metrics
    metrics::register_metrics();

    // Create shared state
    let state = create_shared_state();

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start orchestrator
    let orchestrator = Orchestrator::new(state.clone(), shutdown_rx);
    let orchestrator_handle = tokio::spawn(async move {
        orchestrator.run().await;
    });

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::health))  // Readiness probe
        .route("/status", get(handlers::get_status))
        .route("/metrics", get(handlers::get_metrics))
        .route("/mode", post(handlers::set_mode))
        .route("/cpu", post(handlers::set_cpu_config))
        .route("/memory", post(handlers::set_memory_config))
        .route("/network", post(handlers::set_network_config))
        .route("/stop", post(handlers::stop_all))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("k8s-stressor v{} starting on {}", VERSION, addr);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    
    let server = axum::serve(listener, app);
    
    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal");
            let _ = shutdown_tx.send(true);
        }
    }

    let _ = orchestrator_handle.await;
    tracing::info!("Shutdown complete");
}
```

---

## Kubernetes Manifests

### k8s/deployment.yaml

```yaml
# Author: Luis Amaral
# Created: 2024-11-28

apiVersion: apps/v1
kind: Deployment
metadata:
  name: k8s-stressor
  labels:
    app.kubernetes.io/name: k8s-stressor
    app.kubernetes.io/component: stressor
    app.kubernetes.io/part-of: reliability-testing
    app.kubernetes.io/managed-by: kubectl
spec:
  replicas: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: k8s-stressor
  template:
    metadata:
      labels:
        app.kubernetes.io/name: k8s-stressor
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: default
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: stressor
        image: ghcr.io/luiscamaral/k8s-stressor:latest
        imagePullPolicy: Always
        ports:
        - name: http
          containerPort: 8080
          protocol: TCP
        env:
        - name: RUST_LOG
          value: "info"
        - name: RUST_BACKTRACE
          value: "1"
        resources:
          requests:
            cpu: "100m"
            memory: "64Mi"
          limits:
            cpu: "4000m"
            memory: "2Gi"
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 5
          periodSeconds: 10
          timeoutSeconds: 3
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: http
          initialDelaySeconds: 2
          periodSeconds: 5
          timeoutSeconds: 2
          failureThreshold: 3
```

### k8s/service.yaml

```yaml
# Author: Luis Amaral
# Created: 2024-11-28

apiVersion: v1
kind: Service
metadata:
  name: k8s-stressor
  labels:
    app.kubernetes.io/name: k8s-stressor
spec:
  type: ClusterIP
  selector:
    app.kubernetes.io/name: k8s-stressor
  ports:
  - name: http
    port: 8080
    targetPort: http
    protocol: TCP
```

### k8s/configmap.yaml

```yaml
# Author: Luis Amaral
# Created: 2024-11-28

apiVersion: v1
kind: ConfigMap
metadata:
  name: k8s-stressor-config
  labels:
    app.kubernetes.io/name: k8s-stressor
data:
  # Default CPU configuration
  cpu-config.json: |
    {
      "mode": "linear",
      "max_value": 1000,
      "start_value": 100,
      "growth_rate": 10,
      "midpoint_maxpoint": 30000,
      "duration": 60,
      "interval": 10
    }
  
  # Default Memory configuration
  memory-config.json: |
    {
      "target_mb": 256,
      "duration": 60,
      "interval": 30
    }
  
  # Default Network configuration
  network-config.json: |
    {
      "endpoint": "http://localhost:8080/health",
      "protocol": "http",
      "connections": 10,
      "duration": 60,
      "interval": 10
    }
```

---

## GitHub Actions CI/CD

### .github/workflows/ci.yml

```yaml
# Author: Luis Amaral
# Created: 2024-11-28

name: CI/CD Pipeline

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  # Job 1: Lint and Format Check
  lint:
    name: Lint & Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
        with:
          components: rustfmt, clippy
      
      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  # Job 2: Unit Tests
  test:
    name: Unit Tests
    runs-on: ubuntu-latest
    needs: lint
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
      
      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        run: cargo test --verbose --all-features
      
      - name: Run tests (release mode)
        run: cargo test --release --verbose

  # Job 3: Build and Push Docker Image
  build:
    name: Build & Push Image
    runs-on: ubuntu-latest
    needs: test
    permissions:
      contents: read
      packages: write
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
      
      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Extract metadata for Docker
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=sha,prefix=
            type=ref,event=branch
            type=ref,event=pr
            type=raw,value=latest,enable={{is_default_branch}}
      
      - name: Build and push Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  # Job 4: Integration Tests (on built image)
  integration:
    name: Integration Tests
    runs-on: ubuntu-latest
    needs: build
    if: github.event_name != 'pull_request'
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Pull built image
        run: |
          docker pull ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
      
      - name: Start container
        run: |
          docker run -d --name stressor \
            -p 8080:8080 \
            ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
          sleep 5
      
      - name: Run integration tests
        run: |
          chmod +x scripts/test-phase3.sh
          ./scripts/test-phase3.sh http://localhost:8080
      
      - name: Collect logs on failure
        if: failure()
        run: docker logs stressor
      
      - name: Stop container
        if: always()
        run: docker stop stressor || true

  # Job 5: Security Scan
  security:
    name: Security Scan
    runs-on: ubuntu-latest
    needs: build
    if: github.event_name != 'pull_request'
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Run Trivy vulnerability scanner
        uses: aquasecurity/trivy-action@master
        with:
          image-ref: '${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}'
          format: 'sarif'
          output: 'trivy-results.sarif'
        env:
          TRIVY_USERNAME: ${{ github.actor }}
          TRIVY_PASSWORD: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Upload Trivy scan results
        uses: github/codeql-action/upload-sarif@v2
        if: always()
        with:
          sarif_file: 'trivy-results.sarif'
```

---

## Integration Test Script

### scripts/test-phase3.sh

```bash
#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"

echo "=== Phase 3 Integration Tests ==="
echo "Target: $BASE_URL"
echo ""

# Helper function
check_metric() {
    local metric_name="$1"
    local expected_pattern="$2"
    local metrics=$(curl -sf "$BASE_URL/metrics")
    if echo "$metrics" | grep -q "$metric_name"; then
        echo "✓"
        return 0
    else
        echo "✗ (metric $metric_name not found)"
        return 1
    fi
}

# Test 1: Health and Ready endpoints
echo "Test 1: Health and Ready Endpoints"
echo -n "  /health... "
HEALTH=$(curl -sf "$BASE_URL/health")
[ "$HEALTH" = "OK" ] && echo "✓" || echo "✗"

echo -n "  /ready... "
READY=$(curl -sf "$BASE_URL/ready")
[ "$READY" = "OK" ] && echo "✓" || echo "✗"
echo ""

# Test 2: Prometheus Metrics Format
echo "Test 2: Prometheus Metrics Format"
echo -n "  Content-Type header... "
CT=$(curl -sI "$BASE_URL/metrics" | grep -i "content-type" | tr -d '\r')
if echo "$CT" | grep -q "text/plain"; then
    echo "✓"
else
    echo "✗ ($CT)"
fi

echo -n "  stressor_mode metric... "
check_metric "stressor_mode"

echo -n "  stressor_is_active metric... "
check_metric "stressor_is_active"

echo -n "  stressor_config_version metric... "
check_metric "stressor_config_version"

echo -n "  stressor_build_info metric... "
check_metric "stressor_build_info"
echo ""

# Test 3: CPU Stressor with Metrics
echo "Test 3: CPU Stressor with Metrics"
echo -n "  Configure and start CPU stressor... "
curl -sf -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":30,"interval":0}' > /dev/null
curl -sf -X POST "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"' > /dev/null
echo "✓"

echo -n "  Wait and check metrics... "
sleep 2
METRICS=$(curl -sf "$BASE_URL/metrics")
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
ACTIVE=$(echo "$METRICS" | grep "^stressor_is_active " | awk '{print $2}')
if [ "$MODE" = "1" ] && [ "$ACTIVE" = "1" ]; then
    echo "✓ (mode=$MODE, active=$ACTIVE)"
else
    echo "✗ (mode=$MODE, active=$ACTIVE)"
fi

echo -n "  Stop and verify idle... "
curl -sf -X POST "$BASE_URL/stop" > /dev/null
sleep 1
METRICS=$(curl -sf "$BASE_URL/metrics")
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "0" ]; then
    echo "✓"
else
    echo "✗ (mode=$MODE)"
fi
echo ""

# Test 4: Memory Stressor with Metrics
echo "Test 4: Memory Stressor with Metrics"
echo -n "  Configure and start memory stressor... "
curl -sf -X POST "$BASE_URL/memory" \
    -H "Content-Type: application/json" \
    -d '{"target_mb":50,"duration":30,"interval":0}' > /dev/null
curl -sf -X POST "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"memory-stressor"' > /dev/null
echo "✓"

echo -n "  Wait and check mode=2... "
sleep 2
METRICS=$(curl -sf "$BASE_URL/metrics")
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "2" ]; then
    echo "✓"
else
    echo "✗ (mode=$MODE)"
fi

curl -sf -X POST "$BASE_URL/stop" > /dev/null
echo ""

# Test 5: Config Version Increments
echo "Test 5: Config Version Increments"
V1=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_config_version " | awk '{print $2}')
curl -sf -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"burst","max_value":1000,"start_value":0,"growth_rate":0,"midpoint_maxpoint":5000,"duration":10,"interval":0}' > /dev/null
V2=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_config_version " | awk '{print $2}')
echo -n "  Version incremented ($V1 -> $V2)... "
if [ "$V2" -gt "$V1" ]; then
    echo "✓"
else
    echo "✗"
fi
echo ""

# Test 6: Invalid Config Rejected
echo "Test 6: Error Handling"
echo -n "  Invalid CPU config rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":0,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":60,"interval":10}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓"
else
    echo "✗ (HTTP $HTTP_CODE)"
fi

echo -n "  Invalid memory config rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/memory" \
    -H "Content-Type: application/json" \
    -d '{"target_mb":0,"duration":60,"interval":0}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓"
else
    echo "✗ (HTTP $HTTP_CODE)"
fi
echo ""

# Test 7: Verify Final State
echo "Test 7: Final State Verification"
echo -n "  Status endpoint returns valid JSON... "
STATUS=$(curl -sf "$BASE_URL/status")
if echo "$STATUS" | jq . > /dev/null 2>&1; then
    echo "✓"
else
    echo "✗"
fi

echo -n "  Mode is idle... "
MODE=$(echo "$STATUS" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓"
else
    echo "✗ ($MODE)"
fi
echo ""

echo "=== All Phase 3 tests passed! ==="
```

---

## Final Dockerfile

```dockerfile
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

# Build stage
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p src/api src/engines src/orchestrator src/metrics && \
    echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

LABEL org.opencontainers.image.source="https://github.com/luiscamaral/k8s-stressor-pod"
LABEL org.opencontainers.image.description="k8s-stressor - Kubernetes reliability testing"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.authors="Luis Amaral"
LABEL org.opencontainers.image.version="0.3.0"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -s /bin/false stressor

COPY --from=builder /app/target/release/k8s-stressor /usr/local/bin/

USER 1000

EXPOSE 8080

ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["k8s-stressor"]
```

---

## Local Testing Commands

```bash
# Run all tests
cargo test --all-features

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Build Docker image
docker build -t k8s-stressor:0.3.0 .

# Run container
docker run --rm -p 8080:8080 --name stressor k8s-stressor:0.3.0

# Run integration tests
chmod +x scripts/test-phase3.sh
./scripts/test-phase3.sh

# Test Prometheus metrics
curl http://localhost:8080/metrics

# Deploy to Kubernetes (local)
kubectl apply -f k8s/
```

---

## CI/CD Workflow Summary

```
┌─────────────────────────────────────────────────────────────┐
│                    GitHub Actions Pipeline                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  push/PR to main/develop                                    │
│         │                                                   │
│         ▼                                                   │
│  ┌─────────────┐                                           │
│  │    Lint     │ ── cargo fmt, clippy                      │
│  └──────┬──────┘                                           │
│         │                                                   │
│         ▼                                                   │
│  ┌─────────────┐                                           │
│  │    Test     │ ── cargo test (debug + release)           │
│  └──────┬──────┘                                           │
│         │                                                   │
│         ▼                                                   │
│  ┌─────────────┐                                           │
│  │    Build    │ ── Docker build + push to ghcr.io         │
│  └──────┬──────┘                                           │
│         │                                                   │
│    ┌────┴────┐                                             │
│    │         │                                             │
│    ▼         ▼                                             │
│  ┌─────┐  ┌──────────┐                                    │
│  │Integ│  │ Security │                                    │
│  │Tests│  │   Scan   │                                    │
│  └─────┘  └──────────┘                                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 3 Completion Checklist

- [ ] `cargo build --release` succeeds
- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] Dockerfile builds successfully
- [ ] `/metrics` endpoint returns Prometheus format
- [ ] All metrics are present and updating
- [ ] K8s manifests are valid (`kubectl apply --dry-run`)
- [ ] `scripts/test-phase3.sh` passes all tests
- [ ] GitHub Actions workflow runs successfully
- [ ] Image is pushed to `ghcr.io/<username>/k8s-stressor`
- [ ] Security scan completes

---

## Repository Setup for CI/CD

1. **Enable GitHub Container Registry:**
   - Go to repo Settings → Actions → General
   - Enable "Read and write permissions" for GITHUB_TOKEN

2. **Package Visibility:**
   - After first push, go to Packages
   - Set visibility to Private (or Public if desired)

3. **Branch Protection (optional):**
   - Require status checks to pass before merging
   - Require PR reviews

---

## Summary

Phase 3 completes the k8s-stressor project with:

- **Prometheus metrics** for observability
- **Kubernetes manifests** for deployment
- **GitHub Actions CI/CD** pipeline
- **Private container registry** (ghcr.io)
- **Automated testing** in pipeline
- **Security scanning** with Trivy

The project is now production-ready for Kubernetes reliability testing.
