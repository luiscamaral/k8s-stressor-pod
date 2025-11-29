# Phase 1: API Foundation

> **Author:** Luis Amaral  
> **Version:** 0.1.0  
> **Duration:** 2-3 days  
> **Deliverable:** Working API server with Docker image

---

## Overview

Build the foundation: project structure, data models, and a fully functional REST API that accepts and validates configuration—without executing any stress operations.

### Success Criteria

- [ ] All API endpoints return correct responses
- [ ] Configuration updates persist in state
- [ ] Unit tests pass (100% coverage on config/state)
- [ ] Integration tests pass (API contract)
- [ ] Docker image builds and runs locally
- [ ] Health check responds correctly

---

## Project Structure

```
k8s-stressor/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── .dockerignore
├── README.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── state.rs
│   ├── error.rs
│   └── api/
│       ├── mod.rs
│       └── handlers.rs
└── tests/
    ├── config_tests.rs
    ├── state_tests.rs
    └── api_tests.rs
```

---

## Task Checklist

### 1.1 Project Initialization

```bash
cargo new k8s-stressor
cd k8s-stressor
```

### 1.2 Cargo.toml

```toml
[package]
name = "k8s-stressor"
version = "0.1.0"
edition = "2021"
authors = ["Luis Amaral"]
license = "MIT"
description = "Deterministic resource consumption for Kubernetes reliability testing"

[dependencies]
# Async Runtime
tokio = { version = "1", features = ["full", "sync", "time", "rt-multi-thread"] }

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

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
tokio-test = "0.4"
axum-test = "14"
```

### 1.3 src/error.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::InvalidConfig(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(serde_json::json!({
            "error": message,
        }));

        (status, body).into_response()
    }
}
```

### 1.4 src/config.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use serde::{Deserialize, Serialize};

/// Operation mode - only one active at a time
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OperationMode {
    CpuStressor,
    MemoryStressor,
    NetworkStressor,
    #[default]
    Idle,
}

/// Load curve profile
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CurveMode {
    #[default]
    Linear,
    Burst,
    SCurve,
}

/// CPU stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CpuConfig {
    /// Curve type: linear, burst, or s-curve
    pub mode: CurveMode,
    /// Maximum CPU in milli-cores (e.g., 2000 = 2 cores)
    pub max_value: u32,
    /// Starting CPU in milli-cores
    pub start_value: u32,
    /// Growth rate per second
    pub growth_rate: u32,
    /// Midpoint for s-curve OR duration for burst (milliseconds)
    pub midpoint_maxpoint: u32,
    /// Total duration in seconds
    pub duration: u64,
    /// Rest interval between cycles in seconds
    pub interval: u64,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            mode: CurveMode::Linear,
            max_value: 1000,
            start_value: 100,
            growth_rate: 10,
            midpoint_maxpoint: 30000,
            duration: 60,
            interval: 10,
        }
    }
}

impl CpuConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<(), String> {
        if self.max_value == 0 {
            return Err("max_value must be greater than 0".to_string());
        }
        if self.start_value > self.max_value {
            return Err("start_value cannot exceed max_value".to_string());
        }
        if self.duration == 0 {
            return Err("duration must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Memory stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryConfig {
    /// Target memory allocation in MB
    pub target_mb: u32,
    /// Duration to hold allocation in seconds
    pub duration: u64,
    /// Rest interval between cycles in seconds
    pub interval: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            target_mb: 256,
            duration: 60,
            interval: 30,
        }
    }
}

impl MemoryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_mb == 0 {
            return Err("target_mb must be greater than 0".to_string());
        }
        if self.duration == 0 {
            return Err("duration must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Network stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfig {
    /// Target endpoint URL
    pub endpoint: String,
    /// Protocol: http, tcp, udp
    pub protocol: String,
    /// Number of concurrent connections
    pub connections: u32,
    /// Duration in seconds
    pub duration: u64,
    /// Rest interval in seconds
    pub interval: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            endpoint: String::from("http://localhost:8080/health"),
            protocol: String::from("http"),
            connections: 10,
            duration: 60,
            interval: 10,
        }
    }
}

impl NetworkConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.endpoint.is_empty() {
            return Err("endpoint cannot be empty".to_string());
        }
        if self.connections == 0 {
            return Err("connections must be greater than 0".to_string());
        }
        if !["http", "tcp", "udp"].contains(&self.protocol.as_str()) {
            return Err("protocol must be http, tcp, or udp".to_string());
        }
        Ok(())
    }
}
```

### 1.5 src/state.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};

/// Shared application state
#[derive(Clone, Debug)]
pub struct AppState {
    pub current_mode: OperationMode,
    pub cpu_config: CpuConfig,
    pub memory_config: MemoryConfig,
    pub network_config: NetworkConfig,
    /// Incremented on every config change
    pub config_version: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_mode: OperationMode::Idle,
            cpu_config: CpuConfig::default(),
            memory_config: MemoryConfig::default(),
            network_config: NetworkConfig::default(),
            config_version: 0,
        }
    }
}

impl AppState {
    /// Increment version on state change
    pub fn bump_version(&mut self) {
        self.config_version += 1;
    }
}

/// Thread-safe state wrapper
pub type SharedState = Arc<RwLock<AppState>>;

/// Create new shared state instance
pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}
```

### 1.6 src/api/handlers.rs

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
use crate::state::SharedState;

/// Response for status endpoint
#[derive(serde::Serialize)]
pub struct StatusResponse {
    pub mode: OperationMode,
    pub config_version: u64,
    pub cpu_config: CpuConfig,
    pub memory_config: MemoryConfig,
    pub network_config: NetworkConfig,
}

/// GET /health
pub async fn health() -> &'static str {
    "OK"
}

/// GET /status
pub async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    Json(StatusResponse {
        mode: s.current_mode.clone(),
        config_version: s.config_version,
        cpu_config: s.cpu_config.clone(),
        memory_config: s.memory_config.clone(),
        network_config: s.network_config.clone(),
    })
}

/// GET /metrics (placeholder for Phase 1)
pub async fn get_metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    let mode_num = match s.current_mode {
        OperationMode::Idle => 0,
        OperationMode::CpuStressor => 1,
        OperationMode::MemoryStressor => 2,
        OperationMode::NetworkStressor => 3,
    };

    format!(
        "# HELP stressor_mode Current operation mode (0=idle, 1=cpu, 2=memory, 3=network)\n\
         # TYPE stressor_mode gauge\n\
         stressor_mode {}\n\
         # HELP stressor_config_version Configuration version counter\n\
         # TYPE stressor_config_version counter\n\
         stressor_config_version {}\n\
         # HELP stressor_is_active Whether a stressor is currently running\n\
         # TYPE stressor_is_active gauge\n\
         stressor_is_active {}\n",
        mode_num,
        s.config_version,
        if s.current_mode == OperationMode::Idle { 0 } else { 1 }
    )
}

/// POST /mode
pub async fn set_mode(
    State(state): State<SharedState>,
    Json(mode): Json<OperationMode>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Mode change: {:?} -> {:?}", s.current_mode, mode);
    s.current_mode = mode;
    s.bump_version();
    StatusCode::OK
}

/// POST /cpu
pub async fn set_cpu_config(
    State(state): State<SharedState>,
    Json(config): Json<CpuConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("CPU config updated: {:?}", config);
    s.cpu_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /memory
pub async fn set_memory_config(
    State(state): State<SharedState>,
    Json(config): Json<MemoryConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("Memory config updated: {:?}", config);
    s.memory_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /network
pub async fn set_network_config(
    State(state): State<SharedState>,
    Json(config): Json<NetworkConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("Network config updated: {:?}", config);
    s.network_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /stop
pub async fn stop_all(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    StatusCode::OK
}
```

### 1.7 src/api/mod.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod handlers;
```

### 1.8 src/lib.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod api;
pub mod config;
pub mod error;
pub mod state;
```

### 1.9 src/main.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::net::SocketAddr;

use axum::{routing::{get, post}, Router};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use k8s_stressor::api::handlers;
use k8s_stressor::state::create_shared_state;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create shared state
    let state = create_shared_state();

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health))
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
    tracing::info!("k8s-stressor v0.1.0 starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server failed");
}
```

---

## Unit Tests

### tests/config_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use k8s_stressor::config::{CpuConfig, CurveMode, MemoryConfig, NetworkConfig, OperationMode};

#[test]
fn test_cpu_config_default() {
    let config = CpuConfig::default();
    assert_eq!(config.mode, CurveMode::Linear);
    assert_eq!(config.max_value, 1000);
    assert_eq!(config.start_value, 100);
}

#[test]
fn test_cpu_config_validation_valid() {
    let config = CpuConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_cpu_config_validation_invalid_max() {
    let config = CpuConfig {
        max_value: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cpu_config_validation_start_exceeds_max() {
    let config = CpuConfig {
        start_value: 2000,
        max_value: 1000,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_memory_config_validation() {
    let valid = MemoryConfig::default();
    assert!(valid.validate().is_ok());

    let invalid = MemoryConfig {
        target_mb: 0,
        ..Default::default()
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn test_network_config_validation() {
    let valid = NetworkConfig::default();
    assert!(valid.validate().is_ok());

    let invalid_protocol = NetworkConfig {
        protocol: "ftp".to_string(),
        ..Default::default()
    };
    assert!(invalid_protocol.validate().is_err());
}

#[test]
fn test_operation_mode_serialization() {
    let mode = OperationMode::CpuStressor;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"cpu-stressor\"");

    let deserialized: OperationMode = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, OperationMode::CpuStressor);
}

#[test]
fn test_curve_mode_serialization() {
    let mode = CurveMode::SCurve;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"s-curve\"");
}
```

### tests/state_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use k8s_stressor::config::OperationMode;
use k8s_stressor::state::{create_shared_state, AppState};

#[test]
fn test_app_state_default() {
    let state = AppState::default();
    assert_eq!(state.current_mode, OperationMode::Idle);
    assert_eq!(state.config_version, 0);
}

#[test]
fn test_app_state_bump_version() {
    let mut state = AppState::default();
    assert_eq!(state.config_version, 0);
    state.bump_version();
    assert_eq!(state.config_version, 1);
    state.bump_version();
    assert_eq!(state.config_version, 2);
}

#[tokio::test]
async fn test_shared_state_read_write() {
    let state = create_shared_state();

    // Read initial state
    {
        let s = state.read().await;
        assert_eq!(s.current_mode, OperationMode::Idle);
    }

    // Write new mode
    {
        let mut s = state.write().await;
        s.current_mode = OperationMode::CpuStressor;
        s.bump_version();
    }

    // Verify change
    {
        let s = state.read().await;
        assert_eq!(s.current_mode, OperationMode::CpuStressor);
        assert_eq!(s.config_version, 1);
    }
}
```

### tests/api_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use tower::ServiceExt;

use k8s_stressor::api::handlers;
use k8s_stressor::state::create_shared_state;

fn create_test_app() -> Router {
    let state = create_shared_state();
    Router::new()
        .route("/health", get(handlers::health))
        .route("/status", get(handlers::get_status))
        .route("/metrics", get(handlers::get_metrics))
        .route("/mode", post(handlers::set_mode))
        .route("/cpu", post(handlers::set_cpu_config))
        .route("/stop", post(handlers::stop_all))
        .with_state(state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_status_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/status").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_set_mode() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mode")
                .header("Content-Type", "application/json")
                .body(Body::from("\"cpu-stressor\""))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_set_cpu_config_valid() {
    let app = create_test_app();

    let config = r#"{
        "mode": "linear",
        "max_value": 2000,
        "start_value": 100,
        "growth_rate": 50,
        "midpoint_maxpoint": 30000,
        "duration": 60,
        "interval": 10
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cpu")
                .header("Content-Type", "application/json")
                .body(Body::from(config))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_set_cpu_config_invalid() {
    let app = create_test_app();

    let config = r#"{
        "mode": "linear",
        "max_value": 0,
        "start_value": 100,
        "growth_rate": 50,
        "midpoint_maxpoint": 30000,
        "duration": 60,
        "interval": 10
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cpu")
                .header("Content-Type", "application/json")
                .body(Body::from(config))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_stop_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

---

## Docker

### Dockerfile

```dockerfile
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

# Build stage
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p src/api && echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

LABEL maintainer="Luis Amaral"
LABEL version="0.1.0"
LABEL description="k8s-stressor - Kubernetes reliability testing"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false stressor

COPY --from=builder /app/target/release/k8s-stressor /usr/local/bin/

USER stressor

EXPOSE 8080

ENV RUST_LOG=info

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["k8s-stressor"]
```

### .dockerignore

```
target/
.git/
.gitignore
*.md
.cascade/
generated/
tests/
```

---

## Local Testing

### Build and Run

```bash
# Run unit tests
cargo test

# Build Docker image
docker build -t k8s-stressor:0.1.0 .

# Run container
docker run --rm -p 8080:8080 --name stressor k8s-stressor:0.1.0
```

### Integration Test Script

Create `scripts/test-phase1.sh`:

```bash
#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"

echo "=== Phase 1 Integration Tests ==="
echo "Target: $BASE_URL"
echo ""

# Test 1: Health check
echo -n "Test 1: Health check... "
HEALTH=$(curl -sf "$BASE_URL/health")
if [ "$HEALTH" = "OK" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (got: $HEALTH)"
    exit 1
fi

# Test 2: Status endpoint
echo -n "Test 2: Status endpoint... "
STATUS=$(curl -sf "$BASE_URL/status")
MODE=$(echo "$STATUS" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE)"
    exit 1
fi

# Test 3: Metrics endpoint
echo -n "Test 3: Metrics endpoint... "
METRICS=$(curl -sf "$BASE_URL/metrics")
if echo "$METRICS" | grep -q "stressor_mode"; then
    echo "✓ PASS"
else
    echo "✗ FAIL"
    exit 1
fi

# Test 4: Set CPU config
echo -n "Test 4: Set CPU config... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":2000,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":60,"interval":10}')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 5: Verify config change
echo -n "Test 5: Verify config change... "
MAX_VALUE=$(curl -sf "$BASE_URL/status" | jq '.cpu_config.max_value')
if [ "$MAX_VALUE" = "2000" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (max_value: $MAX_VALUE)"
    exit 1
fi

# Test 6: Set mode
echo -n "Test 6: Set mode to cpu-stressor... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 7: Verify mode change
echo -n "Test 7: Verify mode change... "
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "cpu-stressor" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE)"
    exit 1
fi

# Test 8: Stop
echo -n "Test 8: Stop all... "
curl -sf -X POST "$BASE_URL/stop" > /dev/null
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE)"
    exit 1
fi

# Test 9: Invalid config
echo -n "Test 9: Invalid config rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":0,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":60,"interval":10}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE, expected 400)"
    exit 1
fi

echo ""
echo "=== All Phase 1 tests passed! ==="
```

```bash
chmod +x scripts/test-phase1.sh

# Run tests against local Docker container
./scripts/test-phase1.sh
```

---

## Phase 1 Completion Checklist

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes all tests
- [ ] `docker build -t k8s-stressor:0.1.0 .` succeeds
- [ ] `docker run` starts container
- [ ] `scripts/test-phase1.sh` passes all 9 tests
- [ ] Health check responds with "OK"
- [ ] Config validation rejects invalid values

---

## Next Phase

Proceed to [Phase 2: Stressor Engines](./phase-2-stressor-engines.md) after all checklist items are complete.
