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

    // Signal all tasks to stop
    let _ = shutdown_rx.clone();
    
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
