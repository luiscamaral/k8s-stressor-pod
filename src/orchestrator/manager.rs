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
    CpuHandle, MemoryHandle, NetworkHandle,
};
use crate::state::SharedState;

/// Aggregated metrics from all engines
#[derive(Debug, Default)]
pub struct OrchestratorMetrics {
    pub cpu_target_millicores: AtomicU64,
    pub cpu_active_threads: AtomicU64,
    pub memory_target_bytes: AtomicU64,
    pub memory_allocated_bytes: AtomicU64,
    pub network_active_connections: AtomicU64,
    pub network_requests_total: AtomicU64,
    pub network_errors_total: AtomicU64,
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
                    // Update metrics from active engine
                    self.update_metrics(&active_engine);

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
                        match active_engine {
                            ActiveEngine::Cpu(handle) => handle.stop(),
                            ActiveEngine::Memory(handle) => handle.stop(),
                            ActiveEngine::Network(handle) => handle.stop(),
                            ActiveEngine::None => {}
                        }

                        // Reset metrics
                        self.reset_metrics();

                        // Start new engine based on mode
                        active_engine = match mode {
                            OperationMode::CpuStressor => {
                                let handle = cpu::start_cpu_stressor(cpu_config);
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

    /// Update orchestrator metrics from active engine
    fn update_metrics(&self, engine: &ActiveEngine) {
        match engine {
            ActiveEngine::Cpu(handle) => {
                self.metrics.cpu_target_millicores.store(
                    handle.metrics.target_millicores.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                self.metrics.cpu_active_threads.store(
                    handle.metrics.active_threads.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
            ActiveEngine::Memory(handle) => {
                self.metrics.memory_target_bytes.store(
                    handle.metrics.target_bytes.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                self.metrics.memory_allocated_bytes.store(
                    handle.metrics.allocated_bytes.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
            ActiveEngine::Network(handle) => {
                self.metrics.network_active_connections.store(
                    handle.metrics.active_connections.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                self.metrics.network_requests_total.store(
                    handle.metrics.requests_total.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                self.metrics.network_errors_total.store(
                    handle.metrics.errors_total.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
            ActiveEngine::None => {}
        }
    }

    /// Reset all metrics to zero
    fn reset_metrics(&self) {
        self.metrics.cpu_target_millicores.store(0, Ordering::Relaxed);
        self.metrics.cpu_active_threads.store(0, Ordering::Relaxed);
        self.metrics.memory_target_bytes.store(0, Ordering::Relaxed);
        self.metrics.memory_allocated_bytes.store(0, Ordering::Relaxed);
        self.metrics.network_active_connections.store(0, Ordering::Relaxed);
        self.metrics.network_requests_total.store(0, Ordering::Relaxed);
        self.metrics.network_errors_total.store(0, Ordering::Relaxed);
    }
}
