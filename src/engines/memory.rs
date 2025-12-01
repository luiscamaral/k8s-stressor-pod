// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{CurveMode, MemoryConfig};

/// Metrics from memory stressor
#[derive(Debug, Default)]
pub struct MemoryMetrics {
    pub allocated_bytes: AtomicU64,
    pub target_bytes: AtomicU64,
    pub cycle_count: AtomicU64,
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

    let cycle_ms = (config.midpoint_ms as u64 * 2) + (config.interval * 1000);
    tracing::info!(
        "Starting memory stressor: mode={:?}, {}MB-{}MB, midpoint={}ms, cycle={}ms",
        config.mode,
        config.start_mb,
        config.target_mb,
        config.midpoint_ms,
        cycle_ms
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

/// Memory worker - cycles through allocation patterns
/// Cycles: ramp up → hold at max → ramp down → rest at start → repeat
fn memory_worker(config: MemoryConfig, stop: Arc<AtomicBool>, metrics: Arc<MemoryMetrics>) {
    let start_bytes = config.start_mb as usize * 1024 * 1024;
    let target_bytes = config.target_mb as usize * 1024 * 1024;
    let midpoint_ms = config.midpoint_ms as u64;
    let interval_ms = config.interval * 1000;
    let cycle_duration_ms = (midpoint_ms * 2) + interval_ms;
    let ramp_ms = config.ramp_duration_ms();

    tracing::debug!("Memory worker started, ramp_duration={}ms", ramp_ms);

    // Pre-allocate maximum capacity
    let mut data: Vec<u8> = Vec::with_capacity(target_bytes);
    
    // Initialize to start_mb
    for i in 0..start_bytes {
        data.push((i % 256) as u8);
    }
    metrics.allocated_bytes.store(start_bytes as u64, Ordering::Relaxed);

    let mut cycle_start = Instant::now();
    let mut cycle_count: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        let cycle_elapsed_ms = cycle_start.elapsed().as_millis() as u64;

        // Check if cycle completed, start new cycle
        if cycle_elapsed_ms >= cycle_duration_ms {
            cycle_start = Instant::now();
            cycle_count += 1;
            metrics.cycle_count.store(cycle_count, Ordering::Relaxed);
            tracing::debug!("Memory stressor starting cycle {}", cycle_count + 1);
            continue;
        }

        // Calculate target allocation for current point in cycle
        let target_alloc = calculate_memory_target(
            &config, cycle_elapsed_ms, midpoint_ms, start_bytes, target_bytes, ramp_ms
        );

        let current_alloc = data.len();

        // Adjust allocation
        if target_alloc > current_alloc {
            // Grow: allocate more memory
            let to_add = target_alloc - current_alloc;
            for i in 0..to_add {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                data.push(((current_alloc + i) % 256) as u8);
            }
        } else if target_alloc < current_alloc {
            // Shrink: deallocate memory
            data.truncate(target_alloc);
            data.shrink_to_fit();
        }

        // Update metrics
        metrics.allocated_bytes.store(data.len() as u64, Ordering::Relaxed);

        // Touch memory to prevent swap-out (every 4KB)
        for i in (0..data.len()).step_by(4096) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            data[i] = data[i].wrapping_add(1);
        }

        // Small sleep to avoid tight loop
        thread::sleep(Duration::from_millis(100));
    }

    tracing::info!("Memory stressor stopped, releasing {}MB", data.len() / (1024 * 1024));
    metrics.allocated_bytes.store(0, Ordering::SeqCst);
}

/// Calculate target memory allocation based on cycle position
/// 
/// Timeline: |<-- ramp up -->|<-- hold at max -->|<-- ramp down -->|<-- rest -->|
///           0            ramp_ms    (duration-ramp_ms)        duration    cycle_end
fn calculate_memory_target(
    config: &MemoryConfig,
    elapsed_ms: u64,
    midpoint_ms: u64,
    start_bytes: usize,
    target_bytes: usize,
    ramp_ms: u64,
) -> usize {
    let duration_ms = midpoint_ms * 2;
    
    // Phase boundaries
    let ramp_up_end = ramp_ms;
    let ramp_down_start = duration_ms.saturating_sub(ramp_ms);
    let rest_start = duration_ms;

    if elapsed_ms < ramp_up_end {
        // Phase 1: Ramp up (0 to ramp_ms)
        calculate_memory_ramp_up(config, elapsed_ms, ramp_ms, start_bytes, target_bytes)
    } else if elapsed_ms < ramp_down_start {
        // Phase 2: Hold at max (ramp_ms to duration - ramp_ms)
        target_bytes
    } else if elapsed_ms < rest_start {
        // Phase 3: Ramp down (duration - ramp_ms to duration)
        let ramp_down_elapsed = elapsed_ms - ramp_down_start;
        calculate_memory_ramp_down(config, ramp_down_elapsed, ramp_ms, start_bytes, target_bytes)
    } else {
        // Phase 4: Rest at start (duration to cycle_end)
        start_bytes
    }
}

fn calculate_memory_ramp_up(
    config: &MemoryConfig,
    elapsed_ms: u64,
    ramp_ms: u64,
    start_bytes: usize,
    target_bytes: usize,
) -> usize {
    let delta = target_bytes - start_bytes;
    
    match config.mode {
        CurveMode::Linear => {
            let bytes_per_ms = delta as f64 / ramp_ms as f64;
            let additional = (bytes_per_ms * elapsed_ms as f64) as usize;
            (start_bytes + additional).min(target_bytes)
        }
        CurveMode::Burst => {
            // Instant jump to target
            target_bytes
        }
        CurveMode::SCurve => {
            let progress = (elapsed_ms as f64 / ramp_ms as f64).min(1.0);
            let sigmoid = 1.0 / (1.0 + (-10.0 * (progress - 0.5)).exp());
            start_bytes + (delta as f64 * sigmoid) as usize
        }
    }
}

fn calculate_memory_ramp_down(
    config: &MemoryConfig,
    phase_elapsed_ms: u64,
    ramp_ms: u64,
    start_bytes: usize,
    target_bytes: usize,
) -> usize {
    let delta = target_bytes - start_bytes;
    
    match config.mode {
        CurveMode::Linear => {
            let bytes_per_ms = delta as f64 / ramp_ms as f64;
            let reduction = (bytes_per_ms * phase_elapsed_ms as f64) as usize;
            target_bytes.saturating_sub(reduction).max(start_bytes)
        }
        CurveMode::Burst => {
            // Instant drop to start
            start_bytes
        }
        CurveMode::SCurve => {
            let progress = (phase_elapsed_ms as f64 / ramp_ms as f64).min(1.0);
            let sigmoid = 1.0 / (1.0 + (-10.0 * (progress - 0.5)).exp());
            target_bytes - (delta as f64 * sigmoid) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stressor_start_stop() {
        let config = MemoryConfig {
            mode: CurveMode::Linear,
            target_mb: 10,
            start_mb: 0,
            growth_rate: 10,
            midpoint_ms: 5000,
            interval: 1,
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

    #[test]
    fn test_memory_linear_ramp_up() {
        let config = MemoryConfig {
            mode: CurveMode::Linear,
            target_mb: 100,
            start_mb: 0,
            growth_rate: 10, // 10 MB/s
            midpoint_ms: 30000,
            interval: 10,
        };

        let start_bytes = 0;
        let target_bytes = 100 * 1024 * 1024;
        let ramp_ms = config.ramp_duration_ms(); // 10000ms

        // At t=0, should be at start
        let at_0 = calculate_memory_ramp_up(&config, 0, ramp_ms, start_bytes, target_bytes);
        assert_eq!(at_0, 0);

        // At t=5000ms (half ramp), should be ~50MB
        let at_half = calculate_memory_ramp_up(&config, 5000, ramp_ms, start_bytes, target_bytes);
        let expected_half = 50 * 1024 * 1024;
        assert!((at_half as i64 - expected_half as i64).abs() < 1024 * 1024, "Expected ~50MB, got {}", at_half / (1024 * 1024));

        // At t=10000ms (full ramp), should be at target
        let at_full = calculate_memory_ramp_up(&config, 10000, ramp_ms, start_bytes, target_bytes);
        assert_eq!(at_full, target_bytes);
    }

    #[test]
    fn test_memory_linear_ramp_down() {
        let config = MemoryConfig {
            mode: CurveMode::Linear,
            target_mb: 100,
            start_mb: 0,
            growth_rate: 10, // 10 MB/s
            midpoint_ms: 30000,
            interval: 10,
        };

        let start_bytes = 0;
        let target_bytes = 100 * 1024 * 1024;
        let ramp_ms = config.ramp_duration_ms(); // 10000ms

        // At phase start (t=0 into ramp down), should be at target
        let at_0 = calculate_memory_ramp_down(&config, 0, ramp_ms, start_bytes, target_bytes);
        assert_eq!(at_0, target_bytes);

        // At t=5000ms (half ramp down), should be ~50MB
        let at_half = calculate_memory_ramp_down(&config, 5000, ramp_ms, start_bytes, target_bytes);
        let expected_half = 50 * 1024 * 1024;
        assert!((at_half as i64 - expected_half as i64).abs() < 1024 * 1024, "Expected ~50MB, got {}", at_half / (1024 * 1024));

        // At t=10000ms (full ramp down), should be at start
        let at_full = calculate_memory_ramp_down(&config, 10000, ramp_ms, start_bytes, target_bytes);
        assert_eq!(at_full, start_bytes);
    }

    #[test]
    fn test_memory_burst_mode() {
        let config = MemoryConfig {
            mode: CurveMode::Burst,
            target_mb: 100,
            start_mb: 10,
            growth_rate: 10,
            midpoint_ms: 5000,
            interval: 5,
        };

        let start_bytes = 10 * 1024 * 1024;
        let target_bytes = 100 * 1024 * 1024;
        let ramp_ms = config.ramp_duration_ms();

        // Burst ramp up: instant jump to target
        let ramp_up = calculate_memory_ramp_up(&config, 0, ramp_ms, start_bytes, target_bytes);
        assert_eq!(ramp_up, target_bytes);

        // Burst ramp down: instant drop to start
        let ramp_down = calculate_memory_ramp_down(&config, 0, ramp_ms, start_bytes, target_bytes);
        assert_eq!(ramp_down, start_bytes);
    }

    #[test]
    fn test_memory_scurve_mode() {
        let config = MemoryConfig {
            mode: CurveMode::SCurve,
            target_mb: 100,
            start_mb: 0,
            growth_rate: 10,
            midpoint_ms: 30000,
            interval: 10,
        };

        let start_bytes = 0;
        let target_bytes = 100 * 1024 * 1024;
        let ramp_ms = config.ramp_duration_ms();

        // At half ramp, sigmoid should be ~50%
        let at_half = calculate_memory_ramp_up(&config, ramp_ms / 2, ramp_ms, start_bytes, target_bytes);
        let expected_half = 50 * 1024 * 1024;
        assert!((at_half as i64 - expected_half as i64).abs() < 10 * 1024 * 1024, 
            "S-curve at half ramp should be ~50MB, got {}", at_half / (1024 * 1024));

        // At end of ramp, should approach target
        let at_end = calculate_memory_ramp_up(&config, ramp_ms, ramp_ms, start_bytes, target_bytes);
        assert!(at_end > 90 * 1024 * 1024, "S-curve at end should be >90MB, got {}", at_end / (1024 * 1024));
    }

    #[test]
    fn test_memory_target_calculation_phases() {
        let config = MemoryConfig {
            mode: CurveMode::Linear,
            target_mb: 100,
            start_mb: 10,
            growth_rate: 10, // 10 MB/s → ramp_ms = 9000
            midpoint_ms: 30000, // duration = 60000
            interval: 10,
        };

        let start_bytes = 10 * 1024 * 1024;
        let target_bytes = 100 * 1024 * 1024;
        let midpoint_ms = config.midpoint_ms as u64;
        let ramp_ms = config.ramp_duration_ms(); // 9000ms
        let duration = midpoint_ms * 2; // 60000
        let ramp_down_start = duration - ramp_ms; // 51000

        // Phase 1: Ramp up (0 to ramp_ms = 9s)
        let phase1_start = calculate_memory_target(&config, 0, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase1_start, start_bytes, "Start of ramp up");

        // Phase 2: Hold at max (ramp_ms to ramp_down_start = 9s to 51s)
        let phase2_hold = calculate_memory_target(&config, 20000, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase2_hold, target_bytes, "Holding at max");
        
        let phase2_still_hold = calculate_memory_target(&config, 50000, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase2_still_hold, target_bytes, "Still holding at max");

        // Phase 3: Ramp down (ramp_down_start to duration = 51s to 60s)
        let phase3_start = calculate_memory_target(&config, ramp_down_start, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase3_start, target_bytes, "Start of ramp down");
        
        let phase3_end = calculate_memory_target(&config, duration, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase3_end, start_bytes, "End of ramp down");

        // Phase 4: Rest (duration to cycle end)
        let phase4 = calculate_memory_target(&config, 65000, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase4, start_bytes, "Rest phase");
    }
}
