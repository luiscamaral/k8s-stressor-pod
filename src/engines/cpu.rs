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
