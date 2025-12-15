// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{CurveMode, MemoryConfig};

const BLOCK_SIZE_BYTES: usize = 4 * 1024 * 1024; // 4 MiB blocks
const PAGE_SIZE_BYTES: usize = 4096; // 4 KiB pages

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        let len = buf.len();
        while i + 8 <= len {
            let v = self.next_u64().to_le_bytes();
            buf[i..i + 8].copy_from_slice(&v);
            i += 8;
        }
        if i < len {
            let v = self.next_u64().to_le_bytes();
            let remaining = len - i;
            buf[i..].copy_from_slice(&v[..remaining]);
        }
    }
}

/// Metrics from memory stressor
#[derive(Debug, Default)]
pub struct MemoryMetrics {
    pub allocated_bytes: AtomicU64,
    pub config_target_bytes: AtomicU64,
    pub current_target_bytes: AtomicU64,
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
    metrics
        .config_target_bytes
        .store(target_bytes, Ordering::SeqCst);
    metrics
        .current_target_bytes
        .store(target_bytes, Ordering::SeqCst);

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

    let mut rng = XorShift64::new(0x9E37_79B9_7F4A_7C15);
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut current_alloc: usize = 0;

    // Initialize to start_mb if configured
    if start_bytes > 0 {
        adjust_allocation(
            &mut blocks,
            &mut current_alloc,
            start_bytes,
            &mut rng,
            &stop,
        );
    }

    metrics
        .allocated_bytes
        .store(current_alloc as u64, Ordering::Relaxed);

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
            &config,
            cycle_elapsed_ms,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );

        // Update current target metric (dynamic ramp position)
        metrics
            .current_target_bytes
            .store(target_alloc as u64, Ordering::Relaxed);

        adjust_allocation(
            &mut blocks,
            &mut current_alloc,
            target_alloc,
            &mut rng,
            &stop,
        );

        // Update metrics
        metrics
            .allocated_bytes
            .store(current_alloc as u64, Ordering::Relaxed);

        // Touch memory pages to keep them resident (every 4KB)
        touch_pages(&mut blocks, &stop);

        // Small sleep to avoid tight loop
        thread::sleep(Duration::from_millis(100));
    }

    tracing::info!(
        "Memory stressor stopped, releasing {}MB",
        current_alloc / (1024 * 1024)
    );
    metrics.allocated_bytes.store(0, Ordering::SeqCst);
}

fn adjust_allocation(
    blocks: &mut Vec<Vec<u8>>,
    current_alloc: &mut usize,
    target_alloc: usize,
    rng: &mut XorShift64,
    stop: &Arc<AtomicBool>,
) {
    if target_alloc == *current_alloc {
        return;
    }

    if target_alloc > *current_alloc {
        let mut remaining = target_alloc - *current_alloc;

        // First try to grow the last block up to BLOCK_SIZE_BYTES
        if let Some(last) = blocks.last_mut() {
            let available = BLOCK_SIZE_BYTES.saturating_sub(last.len());
            if available > 0 {
                let grow = remaining.min(available);
                let old_len = last.len();
                last.resize(old_len + grow, 0);
                rng.fill_bytes(&mut last[old_len..]);
                *current_alloc += grow;
                remaining -= grow;
            }
        }

        // Allocate additional blocks as needed
        while remaining > 0 && !stop.load(Ordering::Relaxed) {
            let block_len = remaining.min(BLOCK_SIZE_BYTES);
            let mut block = Vec::with_capacity(BLOCK_SIZE_BYTES);
            block.resize(block_len, 0);
            rng.fill_bytes(&mut block[..]);
            *current_alloc += block_len;
            remaining -= block_len;
            blocks.push(block);
        }
    } else {
        // Shrink: deallocate from the end, dropping full blocks when possible
        let mut excess = *current_alloc - target_alloc;

        while excess > 0 {
            if let Some(last) = blocks.last_mut() {
                if excess >= last.len() {
                    excess -= last.len();
                    *current_alloc -= last.len();
                    blocks.pop();
                } else {
                    let new_len = last.len() - excess;
                    last.truncate(new_len);
                    *current_alloc -= excess;
                    excess = 0;
                }
            } else {
                // Nothing left to free
                *current_alloc = 0;
                break;
            }
        }
    }
}

fn touch_pages(blocks: &mut [Vec<u8>], stop: &Arc<AtomicBool>) {
    for block in blocks.iter_mut() {
        let len = block.len();
        if len == 0 {
            continue;
        }

        for i in (0..len).step_by(PAGE_SIZE_BYTES) {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            block[i] = block[i].wrapping_add(1);
        }
    }
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
        calculate_memory_ramp_down(
            config,
            ramp_down_elapsed,
            ramp_ms,
            start_bytes,
            target_bytes,
        )
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
        assert!(
            (at_half as i64 - expected_half as i64).abs() < 1024 * 1024,
            "Expected ~50MB, got {}",
            at_half / (1024 * 1024)
        );

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
        assert!(
            (at_half as i64 - expected_half as i64).abs() < 1024 * 1024,
            "Expected ~50MB, got {}",
            at_half / (1024 * 1024)
        );

        // At t=10000ms (full ramp down), should be at start
        let at_full =
            calculate_memory_ramp_down(&config, 10000, ramp_ms, start_bytes, target_bytes);
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
        let at_half =
            calculate_memory_ramp_up(&config, ramp_ms / 2, ramp_ms, start_bytes, target_bytes);
        let expected_half = 50 * 1024 * 1024;
        assert!(
            (at_half as i64 - expected_half as i64).abs() < 10 * 1024 * 1024,
            "S-curve at half ramp should be ~50MB, got {}",
            at_half / (1024 * 1024)
        );

        // At end of ramp, should approach target
        let at_end = calculate_memory_ramp_up(&config, ramp_ms, ramp_ms, start_bytes, target_bytes);
        assert!(
            at_end > 90 * 1024 * 1024,
            "S-curve at end should be >90MB, got {}",
            at_end / (1024 * 1024)
        );
    }

    #[test]
    fn test_memory_target_calculation_phases() {
        let config = MemoryConfig {
            mode: CurveMode::Linear,
            target_mb: 100,
            start_mb: 10,
            growth_rate: 10,    // 10 MB/s → ramp_ms = 9000
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
        let phase1_start =
            calculate_memory_target(&config, 0, midpoint_ms, start_bytes, target_bytes, ramp_ms);
        assert_eq!(phase1_start, start_bytes, "Start of ramp up");

        // Phase 2: Hold at max (ramp_ms to ramp_down_start = 9s to 51s)
        let phase2_hold = calculate_memory_target(
            &config,
            20000,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );
        assert_eq!(phase2_hold, target_bytes, "Holding at max");

        let phase2_still_hold = calculate_memory_target(
            &config,
            50000,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );
        assert_eq!(phase2_still_hold, target_bytes, "Still holding at max");

        // Phase 3: Ramp down (ramp_down_start to duration = 51s to 60s)
        let phase3_start = calculate_memory_target(
            &config,
            ramp_down_start,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );
        assert_eq!(phase3_start, target_bytes, "Start of ramp down");

        let phase3_end = calculate_memory_target(
            &config,
            duration,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );
        assert_eq!(phase3_end, start_bytes, "End of ramp down");

        // Phase 4: Rest (duration to cycle end)
        let phase4 = calculate_memory_target(
            &config,
            65000,
            midpoint_ms,
            start_bytes,
            target_bytes,
            ramp_ms,
        );
        assert_eq!(phase4, start_bytes, "Rest phase");
    }
}
