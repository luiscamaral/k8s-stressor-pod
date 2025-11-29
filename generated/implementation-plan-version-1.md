# k8s-stressor Implementation Plan v1

> **Author:** Luis Amaral  
> **Parent Document:** [`technical-specification.md`](./technical-specification.md)  
> **Status:** Planning  
> **Estimated Effort:** 3 phases, ~2-3 weeks

---

## Phase Documents

This implementation is broken into three detailed phases, each with complete code, tests, and Docker images:

| Phase | Document | Version | Deliverable |
|-------|----------|---------|-------------|
| **Phase 1** | [API Foundation](./phase-1-api-foundation.md) | 0.1.0 | REST API + Docker image |
| **Phase 2** | [Stressor Engines](./phase-2-stressor-engines.md) | 0.2.0 | CPU/Memory/Network engines |
| **Phase 3** | [Production + CI/CD](./phase-3-production-cicd.md) | 0.3.0 | Metrics, K8s manifests, GitHub Actions |

Each phase includes:
- ✅ Complete source code with file headers
- ✅ Unit tests
- ✅ Integration test scripts
- ✅ Working Docker image
- ✅ Verification checklist

---

## Table of Contents

1. [Project Structure](#project-structure)
2. [Dependencies](#dependencies)
3. [Data Structures](#data-structures)
4. [Phase 1: The Hollow Shell](#phase-1-the-hollow-shell)
5. [Phase 2: CPU Engine](#phase-2-cpu-engine)
6. [Phase 3: Advanced Features](#phase-3-advanced-features)
7. [Testing Strategy](#testing-strategy)
8. [Containerization](#containerization)

---

## Project Structure

```
k8s-stressor/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── README.md
├── generated/
│   ├── technical-specification.md
│   └── implementation-plan-version-1.md
├── k8s/
│   ├── deployment.yaml
│   ├── service.yaml
│   └── configmap.yaml
└── src/
    ├── main.rs                 # Entry point, server bootstrap
    ├── lib.rs                  # Module exports
    ├── config.rs               # Data structures & defaults
    ├── state.rs                # Shared state management
    ├── api/
    │   ├── mod.rs
    │   ├── routes.rs           # Route definitions
    │   └── handlers.rs         # Request handlers
    ├── orchestrator/
    │   ├── mod.rs
    │   └── manager.rs          # Background task coordinator
    ├── engines/
    │   ├── mod.rs
    │   ├── cpu.rs              # CPU stressor (sync threads)
    │   ├── memory.rs           # Memory stressor (sync thread)
    │   └── network.rs          # Network stressor (async tasks)
    └── metrics/
        ├── mod.rs
        └── prometheus.rs       # Metrics collection & export
```

---

## Dependencies

### Cargo.toml

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

# System Info
num_cpus = "1.16"
sysinfo = "0.30"

# HTTP Client (for network stressor)
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }

# Metrics
prometheus = "0.13"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
tokio-test = "0.4"
```

### Justification

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime with multi-threaded executor |
| `axum` | Modern, ergonomic web framework built on Tower |
| `num_cpus` | CPU topology detection for thread distribution |
| `sysinfo` | System metrics for self-monitoring |
| `reqwest` | HTTP client with connection pooling for network stress |
| `prometheus` | Native Prometheus metrics format |

---

## Data Structures

### src/config.rs

```rust
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuConfig {
    /// Curve type: linear, burst, or s-curve
    pub mode: CurveMode,
    /// Maximum CPU in milli-cores (e.g., 2000 = 2 cores)
    pub max_value: u32,
    /// Starting CPU in milli-cores
    pub start_value: u32,
    /// Growth rate per millisecond
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
            max_value: 1000,      // 1 core
            start_value: 100,    // 0.1 core
            growth_rate: 10,
            midpoint_maxpoint: 30000,
            duration: 60,
            interval: 10,
        }
    }
}

/// Memory stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
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

/// Network stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
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
```

### src/state.rs

```rust
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
    /// Incremented on every config change to signal orchestrator
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

/// Thread-safe state wrapper
pub type SharedState = Arc<RwLock<AppState>>;

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}
```

---

## Phase 1: The Hollow Shell

**Goal:** API server that accepts and echoes configuration  
**Deliverable:** Compiling project with working endpoints  
**Duration:** 2-3 days

### Tasks

- [ ] **1.1** Initialize Cargo project with dependencies
- [ ] **1.2** Create module structure (`src/` layout)
- [ ] **1.3** Implement data structures (`config.rs`, `state.rs`)
- [ ] **1.4** Build Axum router with all endpoints
- [ ] **1.5** Implement handlers that log and return config
- [ ] **1.6** Add basic `/status` endpoint
- [ ] **1.7** Add placeholder `/metrics` endpoint
- [ ] **1.8** Write basic integration tests

### src/main.rs (Phase 1)

```rust
use std::net::SocketAddr;
use axum::{routing::get, Router};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod state;

use crate::state::create_shared_state;

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
        .route("/health", get(|| async { "OK" }))
        .route("/status", get(api::handlers::get_status))
        .route("/metrics", get(api::handlers::get_metrics))
        .route("/mode", axum::routing::post(api::handlers::set_mode))
        .route("/cpu", axum::routing::post(api::handlers::set_cpu_config))
        .route("/memory", axum::routing::post(api::handlers::set_memory_config))
        .route("/network", axum::routing::post(api::handlers::set_network_config))
        .route("/stop", axum::routing::post(api::handlers::stop_all))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Starting k8s-stressor on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### src/api/handlers.rs (Phase 1)

```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};
use crate::state::SharedState;

/// GET /status
pub async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    Json(serde_json::json!({
        "mode": s.current_mode,
        "config_version": s.config_version,
        "cpu_config": s.cpu_config,
        "memory_config": s.memory_config,
        "network_config": s.network_config,
    }))
}

/// GET /metrics (placeholder)
pub async fn get_metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    let mode_num = match s.current_mode {
        OperationMode::Idle => 0,
        OperationMode::CpuStressor => 1,
        OperationMode::MemoryStressor => 2,
        OperationMode::NetworkStressor => 3,
    };
    
    format!(
        "# HELP stressor_mode Current operation mode\n\
         stressor_mode {}\n\
         # HELP stressor_is_active Whether a stressor is running\n\
         stressor_is_active {}\n",
        mode_num,
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
    s.config_version += 1;
    StatusCode::OK
}

/// POST /cpu
pub async fn set_cpu_config(
    State(state): State<SharedState>,
    Json(config): Json<CpuConfig>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("CPU config updated: {:?}", config);
    s.cpu_config = config;
    s.config_version += 1;
    StatusCode::OK
}

/// POST /memory
pub async fn set_memory_config(
    State(state): State<SharedState>,
    Json(config): Json<MemoryConfig>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Memory config updated: {:?}", config);
    s.memory_config = config;
    s.config_version += 1;
    StatusCode::OK
}

/// POST /network
pub async fn set_network_config(
    State(state): State<SharedState>,
    Json(config): Json<NetworkConfig>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Network config updated: {:?}", config);
    s.network_config = config;
    s.config_version += 1;
    StatusCode::OK
}

/// POST /stop
pub async fn stop_all(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.config_version += 1;
    StatusCode::OK
}
```

### Verification Checklist (Phase 1)

```bash
# Build and run
cargo build
cargo run

# In another terminal, test endpoints:
curl http://localhost:8080/health
curl http://localhost:8080/status | jq

# Set CPU config
curl -X POST http://localhost:8080/cpu \
  -H "Content-Type: application/json" \
  -d '{"mode":"linear","max_value":2000,"start_value":100,"growth_rate":10,"midpoint_maxpoint":30000,"duration":60,"interval":10}'

# Check status reflects change
curl http://localhost:8080/status | jq

# Verify metrics endpoint
curl http://localhost:8080/metrics
```

---

## Phase 2: CPU Engine

**Goal:** Functional CPU stressor with Linear curve  
**Deliverable:** Observable CPU usage in `kubectl top pod`  
**Duration:** 3-4 days

### Tasks

- [ ] **2.1** Implement orchestrator manager loop
- [ ] **2.2** Create CPU engine with thread spawning
- [ ] **2.3** Implement PWM-based CPU burning
- [ ] **2.4** Connect orchestrator to API state changes
- [ ] **2.5** Add graceful shutdown via cancellation tokens
- [ ] **2.6** Validate with `kubectl top pod`

### src/engines/cpu.rs

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{CpuConfig, CurveMode};

/// Handle to control running CPU stressor
pub struct CpuHandle {
    stop_flag: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl CpuHandle {
    /// Stop all CPU worker threads
    pub fn stop(self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        for handle in self.threads {
            let _ = handle.join();
        }
    }
}

/// Start CPU stressor with given configuration
pub fn start_cpu_stressor(config: CpuConfig) -> CpuHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let num_threads = num_cpus::get();
    let mut threads = Vec::with_capacity(num_threads);

    tracing::info!(
        "Starting CPU stressor: {} threads, mode={:?}, max={}m",
        num_threads,
        config.mode,
        config.max_value
    );

    for thread_id in 0..num_threads {
        let config = config.clone();
        let stop = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            cpu_worker(thread_id, config, stop);
        });

        threads.push(handle);
    }

    CpuHandle { stop_flag, threads }
}

/// CPU worker thread - burns cycles based on config
fn cpu_worker(thread_id: usize, config: CpuConfig, stop: Arc<AtomicBool>) {
    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration);
    let window = Duration::from_millis(100); // 100ms PWM window
    let num_threads = num_cpus::get() as f64;

    tracing::debug!("CPU worker {} started", thread_id);

    while !stop.load(Ordering::SeqCst) {
        let elapsed = start_time.elapsed();
        
        // Check duration
        if elapsed >= duration {
            tracing::info!("CPU worker {} completed duration", thread_id);
            break;
        }

        let elapsed_ms = elapsed.as_millis() as f64;

        // Calculate target load (total across all cores)
        let total_target_milli = calculate_load(&config, elapsed_ms);
        
        // Per-thread target
        let per_thread_milli = total_target_milli / num_threads;
        
        // PWM duty cycle (0.0 to 1.0)
        let duty_cycle = (per_thread_milli / 1000.0).min(1.0).max(0.0);

        // Apply PWM within window
        let active_time = window.mul_f64(duty_cycle);
        let sleep_time = window - active_time;

        // Burn phase
        burn_cycles(active_time);

        // Sleep phase
        if !sleep_time.is_zero() {
            thread::sleep(sleep_time);
        }
    }

    tracing::debug!("CPU worker {} stopped", thread_id);
}

/// Calculate target load based on curve mode
fn calculate_load(config: &CpuConfig, elapsed_ms: f64) -> f64 {
    match config.mode {
        CurveMode::Linear => {
            let load = config.start_value as f64 + (config.growth_rate as f64 * elapsed_ms / 1000.0);
            load.min(config.max_value as f64)
        }
        CurveMode::Burst => {
            if elapsed_ms < config.midpoint_maxpoint as f64 {
                config.max_value as f64
            } else {
                0.0
            }
        }
        CurveMode::SCurve => {
            let l = config.max_value as f64;
            let k = config.growth_rate as f64 / 10000.0; // scale factor
            let t0 = config.midpoint_maxpoint as f64;
            l / (1.0 + (-k * (elapsed_ms - t0)).exp())
        }
    }
}

/// Burn CPU cycles for specified duration
fn burn_cycles(duration: Duration) {
    let start = Instant::now();
    let mut x: f64 = 1.0;
    
    while start.elapsed() < duration {
        // Tight math loop that compiler can't optimize away
        for _ in 0..1000 {
            x = (x * 1.0000001).sin().cos().abs() + 1.0;
        }
    }
    
    // Prevent optimization
    std::hint::black_box(x);
}
```

### src/orchestrator/manager.rs

```rust
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::interval;

use crate::config::OperationMode;
use crate::engines::cpu::{self, CpuHandle};
use crate::state::SharedState;

/// Orchestrator manages stressor lifecycle
pub struct Orchestrator {
    state: SharedState,
    shutdown_rx: watch::Receiver<bool>,
}

impl Orchestrator {
    pub fn new(state: SharedState, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self { state, shutdown_rx }
    }

    /// Main orchestration loop
    pub async fn run(mut self) {
        let mut check_interval = interval(Duration::from_millis(500));
        let mut last_version: u64 = 0;
        let mut cpu_handle: Option<CpuHandle> = None;

        tracing::info!("Orchestrator started");

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    let state = self.state.read().await;
                    
                    // Check for config changes
                    if state.config_version != last_version {
                        last_version = state.config_version;
                        let mode = state.current_mode.clone();
                        let cpu_config = state.cpu_config.clone();
                        drop(state); // Release lock before spawning

                        // Stop existing stressors
                        if let Some(handle) = cpu_handle.take() {
                            tracing::info!("Stopping existing CPU stressor");
                            handle.stop();
                        }

                        // Start new stressor based on mode
                        match mode {
                            OperationMode::CpuStressor => {
                                cpu_handle = Some(cpu::start_cpu_stressor(cpu_config));
                            }
                            OperationMode::Idle => {
                                tracing::info!("Mode set to Idle");
                            }
                            _ => {
                                tracing::warn!("Mode {:?} not yet implemented", mode);
                            }
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        tracing::info!("Orchestrator shutting down");
                        if let Some(handle) = cpu_handle.take() {
                            handle.stop();
                        }
                        break;
                    }
                }
            }
        }
    }
}
```

### Verification Checklist (Phase 2)

```bash
# Build with release optimizations
cargo build --release

# Run in container or pod
docker run --rm -p 8080:8080 k8s-stressor:latest

# Start CPU stress
curl -X POST http://localhost:8080/mode \
  -H "Content-Type: application/json" \
  -d '"cpu-stressor"'

curl -X POST http://localhost:8080/cpu \
  -H "Content-Type: application/json" \
  -d '{"mode":"linear","max_value":1000,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":120,"interval":0}'

# Monitor CPU (in k8s)
kubectl top pod -l app=k8s-stressor --watch

# Stop stress
curl -X POST http://localhost:8080/stop
```

---

## Phase 3: Advanced Features

**Goal:** Complete feature set with memory, network, and metrics  
**Deliverable:** Production-ready stressor  
**Duration:** 4-5 days

### Tasks

- [ ] **3.1** Implement memory stressor with page dirtying
- [ ] **3.2** Implement network stressor with reqwest
- [ ] **3.3** Add Prometheus metrics collection
- [ ] **3.4** Implement Burst and S-Curve modes
- [ ] **3.5** Add interval/cycling logic
- [ ] **3.6** Create Dockerfile
- [ ] **3.7** Create Kubernetes manifests
- [ ] **3.8** End-to-end testing

### src/engines/memory.rs

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::MemoryConfig;

pub struct MemoryHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MemoryHandle {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

pub fn start_memory_stressor(config: MemoryConfig) -> MemoryHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&stop_flag);

    tracing::info!("Starting memory stressor: {}MB", config.target_mb);

    let thread = thread::spawn(move || {
        memory_worker(config, stop);
    });

    MemoryHandle {
        stop_flag,
        thread: Some(thread),
    }
}

fn memory_worker(config: MemoryConfig, stop: Arc<AtomicBool>) {
    let target_bytes = config.target_mb as usize * 1024 * 1024;
    
    // Allocate memory
    let mut data: Vec<u8> = Vec::with_capacity(target_bytes);
    
    // CRITICAL: Dirty every page to force physical allocation
    // Linux page size is typically 4KB
    tracing::info!("Allocating and dirtying {} bytes", target_bytes);
    
    for i in 0..target_bytes {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        data.push((i % 256) as u8);
        
        // Log progress every 100MB
        if i > 0 && i % (100 * 1024 * 1024) == 0 {
            tracing::debug!("Allocated {}MB", i / (1024 * 1024));
        }
    }

    tracing::info!("Memory allocation complete, holding for {}s", config.duration);

    // Hold allocation for duration
    let duration = Duration::from_secs(config.duration);
    let start = std::time::Instant::now();
    
    while !stop.load(Ordering::SeqCst) && start.elapsed() < duration {
        // Periodically touch memory to prevent swap-out
        for i in (0..data.len()).step_by(4096) {
            data[i] = data[i].wrapping_add(1);
        }
        thread::sleep(Duration::from_secs(1));
    }

    tracing::info!("Memory stressor completed");
    // data is dropped here, freeing memory
}
```

### src/engines/network.rs

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::interval;

use crate::config::NetworkConfig;

pub struct NetworkHandle {
    shutdown_tx: watch::Sender<bool>,
}

impl NetworkHandle {
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(true);
    }
}

pub fn start_network_stressor(config: NetworkConfig) -> NetworkHandle {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tracing::info!(
        "Starting network stressor: {} connections to {}",
        config.connections,
        config.endpoint
    );

    tokio::spawn(async move {
        network_worker(config, shutdown_rx).await;
    });

    NetworkHandle { shutdown_tx }
}

async fn network_worker(config: NetworkConfig, mut shutdown_rx: watch::Receiver<bool>) {
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(config.connections as usize)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    let endpoint = Arc::new(config.endpoint);
    let duration = Duration::from_secs(config.duration);
    let start = tokio::time::Instant::now();

    // Spawn connection tasks
    let mut handles = Vec::new();
    
    for i in 0..config.connections {
        let client = client.clone();
        let endpoint = Arc::clone(&endpoint);
        let mut rx = shutdown_rx.clone();

        let handle = tokio::spawn(async move {
            let mut request_interval = interval(Duration::from_millis(100));
            
            loop {
                tokio::select! {
                    _ = request_interval.tick() => {
                        match client.get(endpoint.as_str()).send().await {
                            Ok(resp) => {
                                tracing::trace!("Connection {}: {}", i, resp.status());
                            }
                            Err(e) => {
                                tracing::warn!("Connection {} error: {}", i, e);
                            }
                        }
                    }
                    _ = rx.changed() => {
                        if *rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for duration or shutdown
    tokio::select! {
        _ = tokio::time::sleep(duration) => {
            tracing::info!("Network stressor duration complete");
        }
        _ = shutdown_rx.changed() => {
            tracing::info!("Network stressor shutdown requested");
        }
    }

    // Cancel all tasks
    for handle in handles {
        handle.abort();
    }

    tracing::info!("Network stressor stopped");
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_load_calculation() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100, // 100m per second
            ..Default::default()
        };

        assert_eq!(calculate_load(&config, 0.0), 100.0);
        assert_eq!(calculate_load(&config, 5000.0), 600.0); // 100 + 100*5
        assert_eq!(calculate_load(&config, 15000.0), 1000.0); // capped at max
    }

    #[test]
    fn test_scurve_load_calculation() {
        let config = CpuConfig {
            mode: CurveMode::SCurve,
            max_value: 1000,
            growth_rate: 100,
            midpoint_maxpoint: 30000,
            ..Default::default()
        };

        let at_midpoint = calculate_load(&config, 30000.0);
        assert!((at_midpoint - 500.0).abs() < 50.0); // ~50% at midpoint
    }
}
```

### Integration Tests

```bash
#!/bin/bash
# test/integration.sh

set -e

BASE_URL="http://localhost:8080"

echo "Testing /health..."
curl -sf "$BASE_URL/health" | grep -q "OK"

echo "Testing /status..."
curl -sf "$BASE_URL/status" | jq -e '.mode == "idle"'

echo "Testing CPU config..."
curl -sf -X POST "$BASE_URL/cpu" \
  -H "Content-Type: application/json" \
  -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":10,"midpoint_maxpoint":30000,"duration":10,"interval":0}'

curl -sf "$BASE_URL/status" | jq -e '.cpu_config.max_value == 500'

echo "Testing mode switch..."
curl -sf -X POST "$BASE_URL/mode" \
  -H "Content-Type: application/json" \
  -d '"cpu-stressor"'

sleep 2
curl -sf "$BASE_URL/metrics" | grep -q "stressor_is_active 1"

echo "Testing stop..."
curl -sf -X POST "$BASE_URL/stop"
curl -sf "$BASE_URL/status" | jq -e '.mode == "idle"'

echo "All tests passed!"
```

---

## Containerization

### Dockerfile

```dockerfile
# Build stage
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Build actual application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/k8s-stressor /usr/local/bin/

EXPOSE 8080

ENV RUST_LOG=info

ENTRYPOINT ["k8s-stressor"]
```

### k8s/deployment.yaml

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: k8s-stressor
  labels:
    app: k8s-stressor
spec:
  replicas: 1
  selector:
    matchLabels:
      app: k8s-stressor
  template:
    metadata:
      labels:
        app: k8s-stressor
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      containers:
      - name: stressor
        image: k8s-stressor:latest
        imagePullPolicy: IfNotPresent
        ports:
        - name: http
          containerPort: 8080
        env:
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            cpu: "100m"
            memory: "64Mi"
          limits:
            cpu: "4000m"
            memory: "2Gi"
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 5
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 2
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: k8s-stressor
spec:
  selector:
    app: k8s-stressor
  ports:
  - port: 8080
    targetPort: http
    name: http
```

---

## Milestone Checkpoints

| Phase | Milestone | Success Criteria |
|-------|-----------|-----------------|
| 1 | API Shell | All endpoints return 200, configs echo correctly |
| 2 | CPU Engine | `kubectl top pod` shows expected CPU usage |
| 3 | Full Suite | Memory visible in metrics, network connections observable |

---

## Open Questions

1. **Persistence:** Should configurations persist across pod restarts? (ConfigMap mounting?)
2. **Multi-mode:** Should multiple stressors run simultaneously, or strictly one-at-a-time?
3. **Authentication:** Does the API need auth, or rely on NetworkPolicy for isolation?
4. **Helm Chart:** Should we create a Helm chart for easier parameterization?

---

## References

- [Technical Specification](./technical-specification.md)
- [Tokio Documentation](https://tokio.rs)
- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples)
- [Prometheus Rust Client](https://github.com/prometheus/client_rust)
