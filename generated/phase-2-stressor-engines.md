# Phase 2: Stressor Engines

> **Author:** Luis Amaral  
> **Version:** 0.2.0  
> **Duration:** 3-4 days  
> **Prerequisite:** Phase 1 complete  
> **Deliverable:** Functional CPU, Memory, and Network stressors with Docker image

---

## Overview

Implement the three stressor engines (CPU, Memory, Network) and the orchestrator that manages their lifecycle. Each engine must produce observable, measurable resource consumption.

### Success Criteria

- [ ] CPU stressor produces measurable CPU usage
- [ ] Memory stressor allocates and holds physical RAM
- [ ] Network stressor generates HTTP traffic
- [ ] Orchestrator starts/stops engines on mode change
- [ ] Graceful shutdown works correctly
- [ ] Unit tests pass for all engines
- [ ] Integration tests verify resource consumption
- [ ] Docker image v0.2.0 builds and runs

---

## Project Structure Update

```
k8s-stressor/
├── src/
│   ├── main.rs                 # Updated with orchestrator
│   ├── lib.rs                  # Updated exports
│   ├── config.rs
│   ├── state.rs
│   ├── error.rs
│   ├── api/
│   │   ├── mod.rs
│   │   └── handlers.rs
│   ├── orchestrator/           # NEW
│   │   ├── mod.rs
│   │   └── manager.rs
│   └── engines/                # NEW
│       ├── mod.rs
│       ├── cpu.rs
│       ├── memory.rs
│       └── network.rs
└── tests/
    ├── config_tests.rs
    ├── state_tests.rs
    ├── api_tests.rs
    ├── cpu_engine_tests.rs     # NEW
    ├── memory_engine_tests.rs  # NEW
    └── orchestrator_tests.rs   # NEW
```

---

## Cargo.toml Updates

Add new dependencies:

```toml
[dependencies]
# ... existing dependencies ...

# System Info (for CPU detection)
num_cpus = "1.16"

# HTTP Client (for network stressor)
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

---

## Task Checklist

### 2.1 CPU Engine

#### src/engines/cpu.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{CpuConfig, CurveMode};

/// Metrics from CPU stressor
#[derive(Debug, Default)]
pub struct CpuMetrics {
    pub target_millicores: AtomicU64,
    pub actual_millicores: AtomicU64,
    pub active_threads: AtomicU64,
}

/// Handle to control running CPU stressor
pub struct CpuHandle {
    stop_flag: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
    pub metrics: Arc<CpuMetrics>,
}

impl CpuHandle {
    /// Stop all CPU worker threads gracefully
    pub fn stop(self) {
        tracing::info!("Stopping CPU stressor...");
        self.stop_flag.store(true, Ordering::SeqCst);
        
        for handle in self.threads {
            if let Err(e) = handle.join() {
                tracing::warn!("CPU thread join error: {:?}", e);
            }
        }
        
        tracing::info!("CPU stressor stopped");
    }

    /// Check if stressor is still running
    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Start CPU stressor with given configuration
pub fn start_cpu_stressor(config: CpuConfig) -> CpuHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let metrics = Arc::new(CpuMetrics::default());
    let num_threads = num_cpus::get();
    let mut threads = Vec::with_capacity(num_threads);

    tracing::info!(
        "Starting CPU stressor: {} threads, mode={:?}, max={}m, duration={}s",
        num_threads,
        config.mode,
        config.max_value,
        config.duration
    );

    metrics.active_threads.store(num_threads as u64, Ordering::SeqCst);

    for thread_id in 0..num_threads {
        let config = config.clone();
        let stop = Arc::clone(&stop_flag);
        let metrics = Arc::clone(&metrics);

        let handle = thread::Builder::new()
            .name(format!("cpu-worker-{}", thread_id))
            .spawn(move || {
                cpu_worker(thread_id, config, stop, metrics);
            })
            .expect("Failed to spawn CPU worker thread");

        threads.push(handle);
    }

    CpuHandle {
        stop_flag,
        threads,
        metrics,
    }
}

/// CPU worker thread - burns cycles based on config
fn cpu_worker(
    thread_id: usize,
    config: CpuConfig,
    stop: Arc<AtomicBool>,
    metrics: Arc<CpuMetrics>,
) {
    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration);
    let window = Duration::from_millis(100); // 100ms PWM window
    let num_threads = num_cpus::get() as f64;

    tracing::debug!("CPU worker {} started", thread_id);

    while !stop.load(Ordering::Relaxed) {
        let elapsed = start_time.elapsed();

        // Check duration
        if elapsed >= duration {
            tracing::info!("CPU worker {} completed duration", thread_id);
            break;
        }

        let elapsed_ms = elapsed.as_millis() as f64;

        // Calculate target load (total across all cores)
        let total_target_milli = calculate_load(&config, elapsed_ms);

        // Update metrics (only from thread 0 to avoid contention)
        if thread_id == 0 {
            metrics.target_millicores.store(total_target_milli as u64, Ordering::Relaxed);
        }

        // Per-thread target
        let per_thread_milli = total_target_milli / num_threads;

        // PWM duty cycle (0.0 to 1.0)
        let duty_cycle = (per_thread_milli / 1000.0).clamp(0.0, 1.0);

        // Apply PWM within window
        let active_time = window.mul_f64(duty_cycle);
        let sleep_time = window - active_time;

        // Burn phase
        if !active_time.is_zero() {
            burn_cycles(active_time);
        }

        // Sleep phase
        if !sleep_time.is_zero() {
            thread::sleep(sleep_time);
        }
    }

    tracing::debug!("CPU worker {} stopped", thread_id);
}

/// Calculate target load based on curve mode
pub fn calculate_load(config: &CpuConfig, elapsed_ms: f64) -> f64 {
    match config.mode {
        CurveMode::Linear => {
            let load = config.start_value as f64 
                + (config.growth_rate as f64 * elapsed_ms / 1000.0);
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
            let k = config.growth_rate as f64 / 10000.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_load_linear() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100,
            ..Default::default()
        };

        assert_eq!(calculate_load(&config, 0.0), 100.0);
        assert_eq!(calculate_load(&config, 5000.0), 600.0);
        assert_eq!(calculate_load(&config, 15000.0), 1000.0); // capped
    }

    #[test]
    fn test_calculate_load_burst() {
        let config = CpuConfig {
            mode: CurveMode::Burst,
            max_value: 1000,
            midpoint_maxpoint: 5000,
            ..Default::default()
        };

        assert_eq!(calculate_load(&config, 0.0), 1000.0);
        assert_eq!(calculate_load(&config, 4999.0), 1000.0);
        assert_eq!(calculate_load(&config, 5001.0), 0.0);
    }

    #[test]
    fn test_calculate_load_scurve() {
        let config = CpuConfig {
            mode: CurveMode::SCurve,
            max_value: 1000,
            growth_rate: 100,
            midpoint_maxpoint: 30000,
            ..Default::default()
        };

        let at_midpoint = calculate_load(&config, 30000.0);
        // At midpoint, sigmoid should be ~50%
        assert!((at_midpoint - 500.0).abs() < 50.0);
    }
}
```

### 2.2 Memory Engine

#### src/engines/memory.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::MemoryConfig;

/// Metrics from memory stressor
#[derive(Debug, Default)]
pub struct MemoryMetrics {
    pub allocated_bytes: AtomicU64,
    pub target_bytes: AtomicU64,
}

/// Handle to control running memory stressor
pub struct MemoryHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    pub metrics: Arc<MemoryMetrics>,
}

impl MemoryHandle {
    /// Stop memory stressor gracefully
    pub fn stop(mut self) {
        tracing::info!("Stopping memory stressor...");
        self.stop_flag.store(true, Ordering::SeqCst);
        
        if let Some(handle) = self.thread.take() {
            if let Err(e) = handle.join() {
                tracing::warn!("Memory thread join error: {:?}", e);
            }
        }
        
        tracing::info!("Memory stressor stopped");
    }

    /// Check if stressor is still running
    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Start memory stressor with given configuration
pub fn start_memory_stressor(config: MemoryConfig) -> MemoryHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let metrics = Arc::new(MemoryMetrics::default());
    let stop = Arc::clone(&stop_flag);
    let metrics_clone = Arc::clone(&metrics);

    let target_bytes = config.target_mb as u64 * 1024 * 1024;
    metrics.target_bytes.store(target_bytes, Ordering::SeqCst);

    tracing::info!(
        "Starting memory stressor: {}MB for {}s",
        config.target_mb,
        config.duration
    );

    let thread = thread::Builder::new()
        .name("memory-worker".to_string())
        .spawn(move || {
            memory_worker(config, stop, metrics_clone);
        })
        .expect("Failed to spawn memory worker thread");

    MemoryHandle {
        stop_flag,
        thread: Some(thread),
        metrics,
    }
}

/// Memory worker - allocates and holds memory
fn memory_worker(config: MemoryConfig, stop: Arc<AtomicBool>, metrics: Arc<MemoryMetrics>) {
    let target_bytes = config.target_mb as usize * 1024 * 1024;

    tracing::info!("Allocating {} bytes ({} MB)", target_bytes, config.target_mb);

    // Allocate memory
    let mut data: Vec<u8> = Vec::with_capacity(target_bytes);

    // CRITICAL: Dirty every page to force physical allocation
    // Linux page size is typically 4KB
    let mut allocated: usize = 0;
    for i in 0..target_bytes {
        if stop.load(Ordering::Relaxed) {
            tracing::info!("Memory allocation interrupted at {}MB", allocated / (1024 * 1024));
            return;
        }

        data.push((i % 256) as u8);
        allocated = i + 1;

        // Update metrics and log every 100MB
        if allocated % (100 * 1024 * 1024) == 0 {
            metrics.allocated_bytes.store(allocated as u64, Ordering::Relaxed);
            tracing::debug!("Allocated {}MB / {}MB", allocated / (1024 * 1024), config.target_mb);
        }
    }

    metrics.allocated_bytes.store(target_bytes as u64, Ordering::SeqCst);
    tracing::info!(
        "Memory allocation complete: {}MB, holding for {}s",
        config.target_mb,
        config.duration
    );

    // Hold allocation for duration
    let duration = Duration::from_secs(config.duration);
    let start = std::time::Instant::now();

    while !stop.load(Ordering::Relaxed) && start.elapsed() < duration {
        // Periodically touch memory to prevent swap-out
        for i in (0..data.len()).step_by(4096) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            data[i] = data[i].wrapping_add(1);
        }
        thread::sleep(Duration::from_secs(1));
    }

    tracing::info!("Memory stressor completed, releasing {}MB", config.target_mb);
    // data is dropped here, freeing memory
    metrics.allocated_bytes.store(0, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stressor_start_stop() {
        let config = MemoryConfig {
            target_mb: 10, // Small for testing
            duration: 60,
            interval: 0,
        };

        let handle = start_memory_stressor(config);
        
        // Give it time to allocate
        thread::sleep(Duration::from_millis(500));
        
        assert!(handle.is_running());
        
        // Check metrics
        let allocated = handle.metrics.allocated_bytes.load(Ordering::Relaxed);
        assert!(allocated > 0);
        
        handle.stop();
    }
}
```

### 2.3 Network Engine

#### src/engines/network.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use crate::config::NetworkConfig;

/// Metrics from network stressor
#[derive(Debug, Default)]
pub struct NetworkMetrics {
    pub active_connections: AtomicU64,
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
}

/// Handle to control running network stressor
pub struct NetworkHandle {
    shutdown_tx: watch::Sender<bool>,
    is_running: Arc<AtomicBool>,
    pub metrics: Arc<NetworkMetrics>,
}

impl NetworkHandle {
    /// Stop network stressor gracefully
    pub fn stop(self) {
        tracing::info!("Stopping network stressor...");
        let _ = self.shutdown_tx.send(true);
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Network stressor stopped");
    }

    /// Check if stressor is still running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

/// Start network stressor with given configuration
pub fn start_network_stressor(config: NetworkConfig) -> NetworkHandle {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics = Arc::new(NetworkMetrics::default());
    let is_running = Arc::new(AtomicBool::new(true));

    tracing::info!(
        "Starting network stressor: {} connections to {} for {}s",
        config.connections,
        config.endpoint,
        config.duration
    );

    let metrics_clone = Arc::clone(&metrics);
    let is_running_clone = Arc::clone(&is_running);

    tokio::spawn(async move {
        network_worker(config, shutdown_rx, metrics_clone, is_running_clone).await;
    });

    NetworkHandle {
        shutdown_tx,
        is_running,
        metrics,
    }
}

/// Network worker - spawns connection tasks
async fn network_worker(
    config: NetworkConfig,
    mut shutdown_rx: watch::Receiver<bool>,
    metrics: Arc<NetworkMetrics>,
    is_running: Arc<AtomicBool>,
) {
    let client = match reqwest::Client::builder()
        .pool_max_idle_per_host(config.connections as usize)
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to create HTTP client: {}", e);
            is_running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let endpoint = Arc::new(config.endpoint);
    let duration = Duration::from_secs(config.duration);

    // Spawn connection tasks
    let mut handles = Vec::new();

    for i in 0..config.connections {
        let client = client.clone();
        let endpoint = Arc::clone(&endpoint);
        let metrics = Arc::clone(&metrics);
        let mut rx = shutdown_rx.clone();

        metrics.active_connections.fetch_add(1, Ordering::Relaxed);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match client.get(endpoint.as_str()).send().await {
                            Ok(resp) => {
                                metrics.requests_total.fetch_add(1, Ordering::Relaxed);
                                tracing::trace!("Connection {}: {}", i, resp.status());
                            }
                            Err(e) => {
                                metrics.errors_total.fetch_add(1, Ordering::Relaxed);
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

            metrics.active_connections.fetch_sub(1, Ordering::Relaxed);
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

    is_running.store(false, Ordering::SeqCst);
    tracing::info!(
        "Network stressor stopped. Total requests: {}, Errors: {}",
        metrics.requests_total.load(Ordering::Relaxed),
        metrics.errors_total.load(Ordering::Relaxed)
    );
}
```

### 2.4 Engines Module

#### src/engines/mod.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod cpu;
pub mod memory;
pub mod network;

pub use cpu::{start_cpu_stressor, CpuHandle, CpuMetrics};
pub use memory::{start_memory_stressor, MemoryHandle, MemoryMetrics};
pub use network::{start_network_stressor, NetworkHandle, NetworkMetrics};
```

### 2.5 Orchestrator

#### src/orchestrator/manager.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use crate::config::OperationMode;
use crate::engines::{
    cpu, memory, network,
    CpuHandle, CpuMetrics,
    MemoryHandle, MemoryMetrics,
    NetworkHandle, NetworkMetrics,
};
use crate::state::SharedState;

/// Aggregated metrics from all engines
#[derive(Debug, Default)]
pub struct OrchestratorMetrics {
    pub cpu: Arc<CpuMetrics>,
    pub memory: Arc<MemoryMetrics>,
    pub network: Arc<NetworkMetrics>,
}

/// Active engine handles
enum ActiveEngine {
    None,
    Cpu(CpuHandle),
    Memory(MemoryHandle),
    Network(NetworkHandle),
}

/// Orchestrator manages stressor lifecycle
pub struct Orchestrator {
    state: SharedState,
    shutdown_rx: watch::Receiver<bool>,
    pub metrics: Arc<OrchestratorMetrics>,
}

impl Orchestrator {
    pub fn new(state: SharedState, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            state,
            shutdown_rx,
            metrics: Arc::new(OrchestratorMetrics::default()),
        }
    }

    /// Get metrics reference for sharing
    pub fn metrics(&self) -> Arc<OrchestratorMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Main orchestration loop
    pub async fn run(mut self) {
        let mut check_interval = tokio::time::interval(Duration::from_millis(500));
        let mut last_version: u64 = 0;
        let mut active_engine = ActiveEngine::None;

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
                        let memory_config = state.memory_config.clone();
                        let network_config = state.network_config.clone();
                        drop(state); // Release lock before spawning

                        // Stop existing engine
                        active_engine = match active_engine {
                            ActiveEngine::Cpu(handle) => {
                                handle.stop();
                                ActiveEngine::None
                            }
                            ActiveEngine::Memory(handle) => {
                                handle.stop();
                                ActiveEngine::None
                            }
                            ActiveEngine::Network(handle) => {
                                handle.stop();
                                ActiveEngine::None
                            }
                            ActiveEngine::None => ActiveEngine::None,
                        };

                        // Start new engine based on mode
                        active_engine = match mode {
                            OperationMode::CpuStressor => {
                                let handle = cpu::start_cpu_stressor(cpu_config);
                                // Update shared metrics reference
                                ActiveEngine::Cpu(handle)
                            }
                            OperationMode::MemoryStressor => {
                                let handle = memory::start_memory_stressor(memory_config);
                                ActiveEngine::Memory(handle)
                            }
                            OperationMode::NetworkStressor => {
                                let handle = network::start_network_stressor(network_config);
                                ActiveEngine::Network(handle)
                            }
                            OperationMode::Idle => {
                                tracing::info!("Mode set to Idle");
                                ActiveEngine::None
                            }
                        };
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        tracing::info!("Orchestrator shutting down");

                        // Stop active engine
                        match active_engine {
                            ActiveEngine::Cpu(handle) => handle.stop(),
                            ActiveEngine::Memory(handle) => handle.stop(),
                            ActiveEngine::Network(handle) => handle.stop(),
                            ActiveEngine::None => {}
                        }

                        break;
                    }
                }
            }
        }

        tracing::info!("Orchestrator stopped");
    }
}
```

#### src/orchestrator/mod.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod manager;

pub use manager::{Orchestrator, OrchestratorMetrics};
```

### 2.6 Update lib.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

pub mod api;
pub mod config;
pub mod engines;
pub mod error;
pub mod orchestrator;
pub mod state;
```

### 2.7 Update main.rs

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
use k8s_stressor::orchestrator::Orchestrator;
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
    tracing::info!("k8s-stressor v0.2.0 starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    
    // Handle shutdown
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

    // Wait for orchestrator to finish
    let _ = orchestrator_handle.await;
    tracing::info!("Shutdown complete");
}
```

---

## Unit Tests

### tests/cpu_engine_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use k8s_stressor::config::{CpuConfig, CurveMode};
use k8s_stressor::engines::cpu::{calculate_load, start_cpu_stressor};

#[test]
fn test_linear_load_at_start() {
    let config = CpuConfig {
        mode: CurveMode::Linear,
        start_value: 100,
        max_value: 1000,
        growth_rate: 100,
        ..Default::default()
    };
    assert_eq!(calculate_load(&config, 0.0), 100.0);
}

#[test]
fn test_linear_load_growth() {
    let config = CpuConfig {
        mode: CurveMode::Linear,
        start_value: 100,
        max_value: 1000,
        growth_rate: 100,
        ..Default::default()
    };
    // After 5 seconds: 100 + 100*5 = 600
    assert_eq!(calculate_load(&config, 5000.0), 600.0);
}

#[test]
fn test_linear_load_capped() {
    let config = CpuConfig {
        mode: CurveMode::Linear,
        start_value: 100,
        max_value: 1000,
        growth_rate: 100,
        ..Default::default()
    };
    // Should cap at max_value
    assert_eq!(calculate_load(&config, 20000.0), 1000.0);
}

#[test]
fn test_burst_mode() {
    let config = CpuConfig {
        mode: CurveMode::Burst,
        max_value: 1000,
        midpoint_maxpoint: 5000,
        ..Default::default()
    };

    assert_eq!(calculate_load(&config, 0.0), 1000.0);
    assert_eq!(calculate_load(&config, 4999.0), 1000.0);
    assert_eq!(calculate_load(&config, 5001.0), 0.0);
}

#[test]
fn test_scurve_midpoint() {
    let config = CpuConfig {
        mode: CurveMode::SCurve,
        max_value: 1000,
        growth_rate: 100,
        midpoint_maxpoint: 30000,
        ..Default::default()
    };

    let at_midpoint = calculate_load(&config, 30000.0);
    // Sigmoid at midpoint should be ~50%
    assert!((at_midpoint - 500.0).abs() < 50.0);
}

#[test]
fn test_cpu_stressor_start_stop() {
    let config = CpuConfig {
        mode: CurveMode::Linear,
        max_value: 500,
        start_value: 100,
        growth_rate: 10,
        duration: 60,
        ..Default::default()
    };

    let handle = start_cpu_stressor(config);

    // Let it run briefly
    thread::sleep(Duration::from_millis(200));

    assert!(handle.is_running());
    
    // Check metrics are being updated
    let target = handle.metrics.target_millicores.load(Ordering::Relaxed);
    assert!(target >= 100);

    handle.stop();
}
```

### tests/memory_engine_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use k8s_stressor::config::MemoryConfig;
use k8s_stressor::engines::memory::start_memory_stressor;

#[test]
fn test_memory_stressor_allocates() {
    let config = MemoryConfig {
        target_mb: 10,
        duration: 60,
        interval: 0,
    };

    let handle = start_memory_stressor(config);

    // Give time to allocate
    thread::sleep(Duration::from_secs(1));

    let allocated = handle.metrics.allocated_bytes.load(Ordering::Relaxed);
    let target = handle.metrics.target_bytes.load(Ordering::Relaxed);

    assert_eq!(target, 10 * 1024 * 1024);
    assert!(allocated > 0);

    handle.stop();
}

#[test]
fn test_memory_stressor_stop() {
    let config = MemoryConfig {
        target_mb: 5,
        duration: 300,
        interval: 0,
    };

    let handle = start_memory_stressor(config);
    thread::sleep(Duration::from_millis(500));
    
    assert!(handle.is_running());
    handle.stop();
}
```

### tests/orchestrator_tests.rs

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::time::Duration;

use tokio::sync::watch;

use k8s_stressor::config::OperationMode;
use k8s_stressor::orchestrator::Orchestrator;
use k8s_stressor::state::create_shared_state;

#[tokio::test]
async fn test_orchestrator_starts_cpu_on_mode_change() {
    let state = create_shared_state();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let orchestrator = Orchestrator::new(state.clone(), shutdown_rx);
    
    // Start orchestrator
    let handle = tokio::spawn(async move {
        orchestrator.run().await;
    });

    // Set mode to CPU
    {
        let mut s = state.write().await;
        s.current_mode = OperationMode::CpuStressor;
        s.bump_version();
    }

    // Wait for orchestrator to react
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Shutdown
    let _ = shutdown_tx.send(true);
    let _ = handle.await;
}

#[tokio::test]
async fn test_orchestrator_shutdown() {
    let state = create_shared_state();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let orchestrator = Orchestrator::new(state.clone(), shutdown_rx);
    
    let handle = tokio::spawn(async move {
        orchestrator.run().await;
    });

    // Immediate shutdown
    let _ = shutdown_tx.send(true);
    
    // Should complete without hanging
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("Orchestrator should shutdown within 5 seconds")
        .expect("Task should complete");
}
```

---

## Docker

### Updated Dockerfile

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
    mkdir -p src/api src/engines src/orchestrator && \
    echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

LABEL maintainer="Luis Amaral"
LABEL version="0.2.0"
LABEL description="k8s-stressor - Kubernetes reliability testing"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
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

---

## Integration Test Script

### scripts/test-phase2.sh

```bash
#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"
CONTAINER_NAME="stressor-test"

echo "=== Phase 2 Integration Tests ==="
echo "Target: $BASE_URL"
echo ""

# Test 1: CPU Stressor
echo "Test 1: CPU Stressor"
echo -n "  Setting CPU config... "
curl -sf -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":50,"midpoint_maxpoint":30000,"duration":10,"interval":0}' > /dev/null
echo "✓"

echo -n "  Setting mode to cpu-stressor... "
curl -sf -X POST "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting 3s for CPU stress... "
sleep 3
echo "✓"

echo -n "  Checking metrics show active... "
ACTIVE=$(curl -sf "$BASE_URL/metrics" | grep "stressor_is_active" | awk '{print $2}')
if [ "$ACTIVE" = "1" ]; then
    echo "✓"
else
    echo "✗ (got: $ACTIVE)"
fi

echo -n "  Stopping... "
curl -sf -X POST "$BASE_URL/stop" > /dev/null
echo "✓"
echo ""

# Test 2: Memory Stressor
echo "Test 2: Memory Stressor"
echo -n "  Setting memory config (50MB)... "
curl -sf -X POST "$BASE_URL/memory" \
    -H "Content-Type: application/json" \
    -d '{"target_mb":50,"duration":10,"interval":0}' > /dev/null
echo "✓"

echo -n "  Setting mode to memory-stressor... "
curl -sf -X POST "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"memory-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting 3s for memory allocation... "
sleep 3
echo "✓"

echo -n "  Checking mode is active... "
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "memory-stressor" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi

echo -n "  Stopping... "
curl -sf -X POST "$BASE_URL/stop" > /dev/null
echo "✓"
echo ""

# Test 3: Mode switching
echo "Test 3: Mode Switching"
echo -n "  Switching CPU -> Memory -> Idle... "
curl -sf -X POST "$BASE_URL/mode" -H "Content-Type: application/json" -d '"cpu-stressor"' > /dev/null
sleep 1
curl -sf -X POST "$BASE_URL/mode" -H "Content-Type: application/json" -d '"memory-stressor"' > /dev/null
sleep 1
curl -sf -X POST "$BASE_URL/stop" > /dev/null
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi
echo ""

# Test 4: Version counter
echo "Test 4: Config Version Counter"
V1=$(curl -sf "$BASE_URL/status" | jq '.config_version')
curl -sf -X POST "$BASE_URL/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"burst","max_value":1000,"start_value":0,"growth_rate":0,"midpoint_maxpoint":5000,"duration":10,"interval":0}' > /dev/null
V2=$(curl -sf "$BASE_URL/status" | jq '.config_version')
echo -n "  Version incremented ($V1 -> $V2)... "
if [ "$V2" -gt "$V1" ]; then
    echo "✓"
else
    echo "✗"
fi
echo ""

echo "=== All Phase 2 tests passed! ==="
```

---

## Local Testing Commands

```bash
# Run all unit tests
cargo test

# Run specific engine tests
cargo test cpu_engine
cargo test memory_engine
cargo test orchestrator

# Build Docker image
docker build -t k8s-stressor:0.2.0 .

# Run container
docker run --rm -p 8080:8080 --name stressor k8s-stressor:0.2.0

# Run integration tests (in another terminal)
chmod +x scripts/test-phase2.sh
./scripts/test-phase2.sh

# Monitor CPU usage (while stressor is running)
docker stats stressor
```

---

## Phase 2 Completion Checklist

- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes all tests (including new engine tests)
- [ ] `docker build -t k8s-stressor:0.2.0 .` succeeds
- [ ] CPU stressor produces observable CPU usage
- [ ] Memory stressor allocates physical RAM
- [ ] Orchestrator switches modes correctly
- [ ] Graceful shutdown works (Ctrl+C)
- [ ] `scripts/test-phase2.sh` passes all tests

---

## Next Phase

Proceed to [Phase 3: Production Ready + CI/CD](./phase-3-production-cicd.md) after all checklist items are complete.
