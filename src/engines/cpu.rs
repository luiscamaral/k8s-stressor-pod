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
    pub cycle_count: AtomicU64,
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

    let cycle_ms = config.cycle_duration_ms();
    tracing::info!(
        "Starting CPU stressor: {} threads, mode={:?}, max={}m, midpoint={}ms, cycle={}ms",
        num_threads,
        config.mode,
        config.max_value,
        config.midpoint_ms,
        cycle_ms
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
/// Cycles: ramp up → hold at max → ramp down → rest at start → repeat
fn cpu_worker(
    thread_id: usize,
    config: CpuConfig,
    stop: Arc<AtomicBool>,
    metrics: Arc<CpuMetrics>,
) {
    let window = Duration::from_millis(100); // 100ms PWM window
    let num_threads = num_cpus::get() as f64;
    let cycle_duration_ms = config.cycle_duration_ms();
    let midpoint_ms = config.midpoint_ms as u64;
    let interval_ms = config.interval * 1000;

    tracing::debug!("CPU worker {} started", thread_id);

    let mut cycle_start = Instant::now();
    let mut cycle_count: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        let cycle_elapsed_ms = cycle_start.elapsed().as_millis() as u64;

        // Check if cycle completed, start new cycle
        if cycle_elapsed_ms >= cycle_duration_ms {
            cycle_start = Instant::now();
            cycle_count += 1;
            if thread_id == 0 {
                metrics.cycle_count.store(cycle_count, Ordering::Relaxed);
                tracing::debug!("CPU stressor starting cycle {}", cycle_count + 1);
            }
            continue;
        }

        // Calculate target load based on position in cycle
        let total_target_milli = calculate_load_cyclic(&config, cycle_elapsed_ms, midpoint_ms, interval_ms);

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

/// Calculate target load for cyclic behavior
/// Phase 1: 0 to midpoint_ms - ramp up (or hold at max if ramp complete)
/// Phase 2: midpoint_ms to 2*midpoint_ms - ramp down (or hold at start if ramp complete)
/// Phase 3: 2*midpoint_ms to cycle_end - rest at start_value
pub fn calculate_load_cyclic(config: &CpuConfig, elapsed_ms: u64, midpoint_ms: u64, _interval_ms: u64) -> f64 {
    let start = config.start_value as f64;
    let max = config.max_value as f64;
    let ramp_ms = config.ramp_duration_ms();
    
    // Phase boundaries
    let phase2_start = midpoint_ms;
    let phase3_start = midpoint_ms * 2;
    
    if elapsed_ms < phase2_start {
        // Phase 1: Ramp up (0 to midpoint)
        calculate_ramp_up(config, elapsed_ms, ramp_ms, start, max)
    } else if elapsed_ms < phase3_start {
        // Phase 2: Ramp down (midpoint to 2*midpoint)
        let phase2_elapsed = elapsed_ms - phase2_start;
        calculate_ramp_down(config, phase2_elapsed, ramp_ms, start, max, midpoint_ms)
    } else {
        // Phase 3: Rest at start_value
        start
    }
}

/// Calculate load during ramp-up phase
fn calculate_ramp_up(config: &CpuConfig, elapsed_ms: u64, ramp_ms: u64, start: f64, max: f64) -> f64 {
    match config.mode {
        CurveMode::Linear => {
            let load = start + (config.growth_rate as f64 * elapsed_ms as f64 / 1000.0);
            load.min(max)
        }
        CurveMode::Burst => {
            // Instant jump to max
            max
        }
        CurveMode::SCurve => {
            // Sigmoid ramp up
            let progress = (elapsed_ms as f64 / ramp_ms as f64).min(1.0);
            let sigmoid = 1.0 / (1.0 + (-10.0 * (progress - 0.5)).exp());
            start + (max - start) * sigmoid
        }
    }
}

/// Calculate load during ramp-down phase
fn calculate_ramp_down(config: &CpuConfig, phase_elapsed_ms: u64, ramp_ms: u64, start: f64, max: f64, _midpoint_ms: u64) -> f64 {
    match config.mode {
        CurveMode::Linear => {
            let load = max - (config.growth_rate as f64 * phase_elapsed_ms as f64 / 1000.0);
            load.max(start)
        }
        CurveMode::Burst => {
            // Instant drop to start
            start
        }
        CurveMode::SCurve => {
            // Sigmoid ramp down (inverted)
            let progress = (phase_elapsed_ms as f64 / ramp_ms as f64).min(1.0);
            let sigmoid = 1.0 / (1.0 + (-10.0 * (progress - 0.5)).exp());
            max - (max - start) * sigmoid
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
    fn test_linear_ramp_up() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100, // 100 milli-cores per second
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;

        // At t=0, should be at start_value
        assert_eq!(calculate_load_cyclic(&config, 0, midpoint, interval), 100.0);
        
        // At t=5s (5000ms), should be start + 5*100 = 600
        assert_eq!(calculate_load_cyclic(&config, 5000, midpoint, interval), 600.0);
        
        // At t=9s (9000ms), should hit max (100 + 9*100 = 1000)
        assert_eq!(calculate_load_cyclic(&config, 9000, midpoint, interval), 1000.0);
        
        // At t=15s, should still be at max (holding until midpoint)
        assert_eq!(calculate_load_cyclic(&config, 15000, midpoint, interval), 1000.0);
    }

    #[test]
    fn test_linear_ramp_down() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;

        // At midpoint (30s), ramp down starts from max
        // At t=30001ms, should start decreasing
        let at_midpoint = calculate_load_cyclic(&config, 30000, midpoint, interval);
        assert_eq!(at_midpoint, 1000.0); // Still at max at exact midpoint
        
        // At t=35s (35000ms), should be max - 5*100 = 500
        assert_eq!(calculate_load_cyclic(&config, 35000, midpoint, interval), 500.0);
        
        // At t=39s (39000ms), should be at start_value
        assert_eq!(calculate_load_cyclic(&config, 39000, midpoint, interval), 100.0);
    }

    #[test]
    fn test_linear_rest_phase() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;

        // Phase 3 starts at 2*midpoint = 60000ms
        // Should rest at start_value
        assert_eq!(calculate_load_cyclic(&config, 60000, midpoint, interval), 100.0);
        assert_eq!(calculate_load_cyclic(&config, 65000, midpoint, interval), 100.0);
    }

    #[test]
    fn test_burst_mode() {
        let config = CpuConfig {
            mode: CurveMode::Burst,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 5000, // 5s at max
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;

        // Phase 1: instant max
        assert_eq!(calculate_load_cyclic(&config, 0, midpoint, interval), 1000.0);
        assert_eq!(calculate_load_cyclic(&config, 4999, midpoint, interval), 1000.0);
        
        // Phase 2: instant drop to start
        assert_eq!(calculate_load_cyclic(&config, 5000, midpoint, interval), 100.0);
        assert_eq!(calculate_load_cyclic(&config, 9999, midpoint, interval), 100.0);
        
        // Phase 3: rest at start
        assert_eq!(calculate_load_cyclic(&config, 10000, midpoint, interval), 100.0);
    }

    #[test]
    fn test_scurve_ramp_up() {
        let config = CpuConfig {
            mode: CurveMode::SCurve,
            start_value: 0,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;
        let ramp_ms = config.ramp_duration_ms();

        // At half of ramp duration, sigmoid should be ~50%
        let at_half_ramp = calculate_load_cyclic(&config, ramp_ms / 2, midpoint, interval);
        assert!((at_half_ramp - 500.0).abs() < 100.0, "S-curve at half ramp should be ~50%, got {}", at_half_ramp);

        // At end of ramp, should approach max
        let at_end_ramp = calculate_load_cyclic(&config, ramp_ms, midpoint, interval);
        assert!(at_end_ramp > 900.0, "S-curve at end of ramp should be >90%, got {}", at_end_ramp);
    }

    #[test]
    fn test_scurve_ramp_down() {
        let config = CpuConfig {
            mode: CurveMode::SCurve,
            start_value: 0,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;
        let ramp_ms = config.ramp_duration_ms();

        // At midpoint (start of ramp down), should be near max (s-curve approaches but doesn't hit exact)
        let at_midpoint = calculate_load_cyclic(&config, midpoint, midpoint, interval);
        assert!(at_midpoint > 990.0, "S-curve at midpoint should be >99%, got {}", at_midpoint);

        // At half of ramp down, sigmoid should be ~50%
        let at_half_down = calculate_load_cyclic(&config, midpoint + ramp_ms / 2, midpoint, interval);
        assert!((at_half_down - 500.0).abs() < 100.0, "S-curve ramp down at half should be ~50%, got {}", at_half_down);
    }

    #[test]
    fn test_full_cycle_linear() {
        let config = CpuConfig {
            mode: CurveMode::Linear,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100, // 100 milli-cores/s → 9s ramp
            midpoint_ms: 30000,
            interval: 10,
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;
        let ramp_ms = config.ramp_duration_ms(); // 9000ms

        // Phase 1: Ramp up (0 to midpoint)
        assert_eq!(calculate_load_cyclic(&config, 0, midpoint, interval), 100.0, "Start of ramp up");
        assert_eq!(calculate_load_cyclic(&config, ramp_ms, midpoint, interval), 1000.0, "End of ramp up");
        assert_eq!(calculate_load_cyclic(&config, 20000, midpoint, interval), 1000.0, "Holding at max");

        // Phase 2: Ramp down (midpoint to 2*midpoint)
        assert_eq!(calculate_load_cyclic(&config, midpoint, midpoint, interval), 1000.0, "Start of ramp down");
        assert_eq!(calculate_load_cyclic(&config, midpoint + ramp_ms, midpoint, interval), 100.0, "End of ramp down");
        assert_eq!(calculate_load_cyclic(&config, 50000, midpoint, interval), 100.0, "Holding at start");

        // Phase 3: Rest (2*midpoint to cycle end)
        assert_eq!(calculate_load_cyclic(&config, 60000, midpoint, interval), 100.0, "Rest phase start");
        assert_eq!(calculate_load_cyclic(&config, 65000, midpoint, interval), 100.0, "Rest phase middle");
    }

    #[test]
    fn test_full_cycle_burst() {
        let config = CpuConfig {
            mode: CurveMode::Burst,
            start_value: 100,
            max_value: 1000,
            growth_rate: 100,
            midpoint_ms: 5000, // 5s burst
            interval: 5, // 5s rest
        };

        let midpoint = config.midpoint_ms as u64;
        let interval = config.interval * 1000;

        // Phase 1: Instant max (0 to midpoint)
        assert_eq!(calculate_load_cyclic(&config, 0, midpoint, interval), 1000.0, "Burst start");
        assert_eq!(calculate_load_cyclic(&config, 4999, midpoint, interval), 1000.0, "Burst end");

        // Phase 2: Instant drop to start (midpoint to 2*midpoint)
        assert_eq!(calculate_load_cyclic(&config, 5000, midpoint, interval), 100.0, "Valley start");
        assert_eq!(calculate_load_cyclic(&config, 9999, midpoint, interval), 100.0, "Valley end");

        // Phase 3: Rest at start (2*midpoint to cycle end)
        assert_eq!(calculate_load_cyclic(&config, 10000, midpoint, interval), 100.0, "Rest phase");
    }
}
