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
    pub cycle_count: AtomicU64,
    pub target_connections: AtomicU64,
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

    // Set target connections
    metrics
        .target_connections
        .store(config.connections as u64, Ordering::Relaxed);

    let cycle_ms = (config.midpoint_ms as u64 * 2) + (config.interval * 1000);
    tracing::info!(
        "Starting network stressor: {} connections to {}, active={}ms, cycle={}ms",
        config.connections,
        config.endpoint,
        config.midpoint_ms,
        cycle_ms
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

/// Network worker - spawns connection tasks with cycling behavior
/// Cycles: active (midpoint_ms * 2) → rest (interval) → repeat
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

    let endpoint = Arc::new(config.endpoint.clone());
    let active_duration = Duration::from_millis(config.midpoint_ms as u64 * 2);
    let rest_duration = Duration::from_secs(config.interval);

    loop {
        // Check for shutdown
        if *shutdown_rx.borrow() {
            break;
        }

        let cycle_count = metrics.cycle_count.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::debug!("Network stressor starting cycle {}", cycle_count);

        // Active phase: spawn connection tasks
        let mut handles = Vec::new();
        let active_shutdown = Arc::new(AtomicBool::new(false));

        for i in 0..config.connections {
            let client = client.clone();
            let endpoint = Arc::clone(&endpoint);
            let metrics = Arc::clone(&metrics);
            let shutdown = Arc::clone(&active_shutdown);

            metrics.active_connections.fetch_add(1, Ordering::Relaxed);

            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(100));

                while !shutdown.load(Ordering::Relaxed) {
                    interval.tick().await;

                    match client.get(endpoint.as_str()).send().await {
                        Ok(resp) => {
                            metrics.requests_total.fetch_add(1, Ordering::Relaxed);
                            tracing::trace!("Connection {}: {}", i, resp.status());
                        }
                        Err(e) => {
                            metrics.errors_total.fetch_add(1, Ordering::Relaxed);
                            tracing::trace!("Connection {} error: {}", i, e);
                        }
                    }
                }

                metrics.active_connections.fetch_sub(1, Ordering::Relaxed);
            });

            handles.push(handle);
        }

        // Wait for active duration or shutdown
        tokio::select! {
            _ = tokio::time::sleep(active_duration) => {
                tracing::debug!("Network active phase complete");
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("Network stressor shutdown requested");
                    active_shutdown.store(true, Ordering::SeqCst);
                    for handle in handles {
                        handle.abort();
                    }
                    break;
                }
            }
        }

        // Stop active connections
        active_shutdown.store(true, Ordering::SeqCst);
        for handle in handles {
            let _ = handle.await;
        }

        // Check for shutdown before rest phase
        if *shutdown_rx.borrow() {
            break;
        }

        // Rest phase
        if rest_duration > Duration::ZERO {
            tracing::debug!("Network rest phase: {}s", config.interval);
            tokio::select! {
                _ = tokio::time::sleep(rest_duration) => {}
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }

    is_running.store(false, Ordering::SeqCst);
    tracing::info!(
        "Network stressor stopped. Total requests: {}, Errors: {}, Cycles: {}",
        metrics.requests_total.load(Ordering::Relaxed),
        metrics.errors_total.load(Ordering::Relaxed),
        metrics.cycle_count.load(Ordering::Relaxed)
    );
}
