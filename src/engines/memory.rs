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
