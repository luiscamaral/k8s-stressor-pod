// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-12-10
// License: MIT

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::{CurveMode, DiskConfig, IoPattern};

/// Disk I/O metrics
pub struct DiskMetrics {
    pub target_mbps: AtomicU32,
    pub actual_mbps: AtomicU32,
    pub bytes_written: AtomicU64,
    pub bytes_read: AtomicU64,
    pub io_errors: AtomicU64,
    pub cycle_count: AtomicU64,
}

impl DiskMetrics {
    pub fn new() -> Self {
        Self {
            target_mbps: AtomicU32::new(0),
            actual_mbps: AtomicU32::new(0),
            bytes_written: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            io_errors: AtomicU64::new(0),
            cycle_count: AtomicU64::new(0),
        }
    }
}

/// Handle to control disk stressor
pub struct DiskHandle {
    stop_flag: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl DiskHandle {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DiskHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Start disk I/O stressor
pub fn start_disk_stressor(config: DiskConfig) -> DiskHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    let metrics = Arc::new(DiskMetrics::new());
    let metrics_clone = metrics.clone();

    let thread_handle = thread::spawn(move || {
        disk_worker(config, metrics_clone, stop_flag_clone);
    });

    DiskHandle {
        stop_flag,
        thread_handle: Some(thread_handle),
    }
}

/// Main disk worker thread
fn disk_worker(config: DiskConfig, metrics: Arc<DiskMetrics>, stop_flag: Arc<AtomicBool>) {
    tracing::info!("Disk stressor starting with config: {:?}", config);

    // Create temp files for all paths
    let mut files = Vec::new();
    for path in config.all_paths() {
        match create_temp_file(&path, config.max_file_size_mb) {
            Ok(file_path) => {
                tracing::info!("Created temp file: {:?}", file_path);
                files.push(file_path);
            }
            Err(e) => {
                tracing::error!("Failed to create temp file in {}: {}", path, e);
                metrics.io_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    if files.is_empty() {
        tracing::error!("No temp files created, disk stressor cannot run");
        return;
    }

    let block_size = (config.block_size_kb as usize) * 1024;
    let mut cycle = 0u64;

    while !stop_flag.load(Ordering::Relaxed) {
        cycle += 1;
        metrics.cycle_count.store(cycle, Ordering::Relaxed);

        tracing::debug!("Starting disk I/O cycle {}", cycle);

        // Run one complete cycle
        run_disk_cycle(
            &config,
            &files,
            block_size,
            &metrics,
            &stop_flag,
        );

        // Rest interval
        if config.interval > 0 && !stop_flag.load(Ordering::Relaxed) {
            tracing::debug!("Resting for {} seconds", config.interval);
            thread::sleep(Duration::from_secs(config.interval));
        }
    }

    // Cleanup temp files
    for file_path in files {
        if let Err(e) = fs::remove_file(&file_path) {
            tracing::warn!("Failed to remove temp file {:?}: {}", file_path, e);
        } else {
            tracing::info!("Removed temp file: {:?}", file_path);
        }
    }

    tracing::info!("Disk stressor stopped after {} cycles", cycle);
}

/// Run one complete disk I/O cycle
fn run_disk_cycle(
    config: &DiskConfig,
    files: &[PathBuf],
    block_size: usize,
    metrics: &Arc<DiskMetrics>,
    stop_flag: &Arc<AtomicBool>,
) {
    let cycle_start = Instant::now();
    let midpoint_duration = Duration::from_millis(config.midpoint_ms as u64);
    let total_active_duration = midpoint_duration * 2;

    let mut elapsed = Duration::ZERO;

    while elapsed < total_active_duration && !stop_flag.load(Ordering::Relaxed) {
        let progress = elapsed.as_millis() as f64 / total_active_duration.as_millis() as f64;

        // Calculate target throughput based on curve mode
        let target_mbps = calculate_disk_target(
            config.mode,
            config.start_mbps,
            config.target_mbps,
            progress,
            config.midpoint_ms as f64 / total_active_duration.as_millis() as f64,
        );

        metrics.target_mbps.store(target_mbps, Ordering::Relaxed);

        // Perform I/O operations
        let iteration_start = Instant::now();
        let bytes_per_iteration = (target_mbps as usize) * 1024 * 1024 / 10; // 100ms iterations

        for file_path in files {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let bytes_for_this_file = bytes_per_iteration / files.len();
            perform_io_operations(
                file_path,
                bytes_for_this_file,
                block_size,
                &config.pattern,
                config.read_ratio,
                config.max_file_size_mb,
                metrics,
            );
        }

        // Calculate actual throughput
        let iteration_duration = iteration_start.elapsed();
        if iteration_duration.as_secs_f64() > 0.0 {
            let actual_mbps = (bytes_per_iteration as f64 / iteration_duration.as_secs_f64()) / (1024.0 * 1024.0);
            metrics.actual_mbps.store(actual_mbps as u32, Ordering::Relaxed);
        }

        // Sleep to maintain target rate (100ms iterations)
        let sleep_time = Duration::from_millis(100).saturating_sub(iteration_duration);
        if sleep_time > Duration::ZERO {
            thread::sleep(sleep_time);
        }

        elapsed = cycle_start.elapsed();
    }
}

/// Perform I/O operations on a single file
fn perform_io_operations(
    file_path: &PathBuf,
    target_bytes: usize,
    block_size: usize,
    pattern: &IoPattern,
    read_ratio: f32,
    max_file_size_mb: u32,
    metrics: &Arc<DiskMetrics>,
) {
    let max_file_size = (max_file_size_mb as u64) * 1024 * 1024;
    let mut bytes_processed = 0;

    while bytes_processed < target_bytes {
        let remaining = target_bytes - bytes_processed;
        let chunk_size = remaining.min(block_size);

        // Decide read or write based on ratio
        let do_read = rand::random::<f32>() < read_ratio;

        if do_read {
            // Read operation
            match read_block(file_path, chunk_size, pattern, max_file_size) {
                Ok(bytes_read) => {
                    metrics.bytes_read.fetch_add(bytes_read as u64, Ordering::Relaxed);
                    bytes_processed += bytes_read;
                }
                Err(e) => {
                    tracing::debug!("Read error: {}", e);
                    metrics.io_errors.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            }
        } else {
            // Write operation
            match write_block(file_path, chunk_size, pattern, max_file_size) {
                Ok(bytes_written) => {
                    metrics.bytes_written.fetch_add(bytes_written as u64, Ordering::Relaxed);
                    bytes_processed += bytes_written;
                }
                Err(e) => {
                    tracing::debug!("Write error: {}", e);
                    metrics.io_errors.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
}

/// Read a block from file
fn read_block(
    file_path: &PathBuf,
    size: usize,
    pattern: &IoPattern,
    max_file_size: u64,
) -> io::Result<usize> {
    let mut file = File::open(file_path)?;
    let file_size = file.metadata()?.len();

    if file_size == 0 {
        return Ok(0);
    }

    // Determine read position
    let position = match pattern {
        IoPattern::Sequential => 0, // Always read from start for simplicity
        IoPattern::Random => {
            let max_pos = file_size.saturating_sub(size as u64);
            if max_pos > 0 {
                (rand::random::<u64>() % max_pos).min(max_file_size)
            } else {
                0
            }
        }
    };

    file.seek(SeekFrom::Start(position))?;

    let mut buffer = vec![0u8; size];
    let bytes_read = file.read(&mut buffer)?;

    Ok(bytes_read)
}

/// Write a block to file
fn write_block(
    file_path: &PathBuf,
    size: usize,
    pattern: &IoPattern,
    max_file_size: u64,
) -> io::Result<usize> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(file_path)?;

    let file_size = file.metadata()?.len();

    // Determine write position
    let position = match pattern {
        IoPattern::Sequential => file_size.min(max_file_size.saturating_sub(size as u64)),
        IoPattern::Random => {
            let max_pos = max_file_size.saturating_sub(size as u64);
            if max_pos > 0 {
                rand::random::<u64>() % max_pos
            } else {
                0
            }
        }
    };

    file.seek(SeekFrom::Start(position))?;

    // Write random data
    let buffer = vec![rand::random::<u8>(); size];
    let bytes_written = file.write(&buffer)?;

    // Sync to ensure data hits disk
    file.sync_data()?;

    Ok(bytes_written)
}

/// Create temp file in specified directory
fn create_temp_file(work_dir: &str, max_size_mb: u32) -> io::Result<PathBuf> {
    // Ensure directory exists
    fs::create_dir_all(work_dir)?;

    let file_path = PathBuf::from(work_dir).join(format!("k8s-stressor-{}.tmp", rand::random::<u32>()));

    // Create file with initial size
    let file = File::create(&file_path)?;
    let initial_size = (max_size_mb as u64 / 10) * 1024 * 1024; // Start with 10% of max size
    file.set_len(initial_size)?;
    file.sync_all()?;

    Ok(file_path)
}

/// Calculate target disk throughput based on curve mode
fn calculate_disk_target(
    mode: CurveMode,
    start_mbps: u32,
    target_mbps: u32,
    progress: f64,
    midpoint: f64,
) -> u32 {
    match mode {
        CurveMode::Linear => {
            if progress < midpoint {
                // Ramp up
                let ramp_progress = progress / midpoint;
                start_mbps + ((target_mbps - start_mbps) as f64 * ramp_progress) as u32
            } else {
                // Ramp down
                let ramp_progress = (progress - midpoint) / (1.0 - midpoint);
                target_mbps - ((target_mbps - start_mbps) as f64 * ramp_progress) as u32
            }
        }
        CurveMode::Burst => {
            // Instant jump to max, hold, then drop
            if progress < midpoint {
                target_mbps
            } else {
                start_mbps
            }
        }
        CurveMode::SCurve => {
            if progress < midpoint {
                // S-curve ramp up
                let t = progress / midpoint;
                let sigmoid = 1.0 / (1.0 + (-10.0 * (t - 0.5)).exp());
                start_mbps + ((target_mbps - start_mbps) as f64 * sigmoid) as u32
            } else {
                // S-curve ramp down
                let t = (progress - midpoint) / (1.0 - midpoint);
                let sigmoid = 1.0 / (1.0 + (-10.0 * (t - 0.5)).exp());
                target_mbps - ((target_mbps - start_mbps) as f64 * sigmoid) as u32
            }
        }
    }
}

/// Simple random number generator (for minimal dependencies)
mod rand {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static RNG_STATE: Cell<u64> = Cell::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
    }

    fn next_u64() -> u64 {
        RNG_STATE.with(|state| {
            let mut x = state.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            state.set(x);
            x
        })
    }

    pub fn random<T: RandomValue>() -> T {
        T::from_random(next_u64())
    }

    pub trait RandomValue {
        fn from_random(val: u64) -> Self;
    }

    impl RandomValue for u8 {
        fn from_random(val: u64) -> Self {
            val as u8
        }
    }

    impl RandomValue for u32 {
        fn from_random(val: u64) -> Self {
            val as u32
        }
    }

    impl RandomValue for u64 {
        fn from_random(val: u64) -> Self {
            val
        }
    }

    impl RandomValue for f32 {
        fn from_random(val: u64) -> Self {
            (val as f64 / u64::MAX as f64) as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_metrics_new() {
        let metrics = DiskMetrics::new();
        assert_eq!(metrics.target_mbps.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.actual_mbps.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.bytes_written.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.bytes_read.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_calculate_disk_target_linear() {
        // Ramp up phase
        let target = calculate_disk_target(CurveMode::Linear, 10, 100, 0.25, 0.5);
        assert!(target > 10 && target < 100);

        // Peak
        let target = calculate_disk_target(CurveMode::Linear, 10, 100, 0.5, 0.5);
        assert_eq!(target, 100);

        // Ramp down phase
        let target = calculate_disk_target(CurveMode::Linear, 10, 100, 0.75, 0.5);
        assert!(target > 10 && target < 100);
    }

    #[test]
    fn test_calculate_disk_target_burst() {
        let target = calculate_disk_target(CurveMode::Burst, 10, 100, 0.25, 0.5);
        assert_eq!(target, 100);

        let target = calculate_disk_target(CurveMode::Burst, 10, 100, 0.75, 0.5);
        assert_eq!(target, 10);
    }

    #[test]
    fn test_disk_stressor_start_stop() {
        let config = DiskConfig {
            target_mbps: 1,
            start_mbps: 1,
            midpoint_ms: 1000,
            interval: 0,
            ..Default::default()
        };

        let handle = start_disk_stressor(config);
        std::thread::sleep(Duration::from_millis(100));
        handle.stop();
    }
}
