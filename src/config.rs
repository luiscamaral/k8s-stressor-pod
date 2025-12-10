// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use utoipa::ToSchema;

/// Safe mode limits to prevent accidental cluster damage.
/// These limits are enforced when STRESSOR_SAFE_MODE=true (default).
pub mod safe_mode {
    /// Maximum CPU in millicores when safe mode is enabled (2 cores)
    pub const MAX_CPU_MILLICORES: u32 = 2000;
    /// Maximum memory in MB when safe mode is enabled (2 GB)
    pub const MAX_MEMORY_MB: u32 = 2048;
    /// Maximum concurrent connections when safe mode is enabled
    pub const MAX_NETWORK_CONNECTIONS: u32 = 100;
    /// Maximum termination delay in seconds when safe mode is enabled
    pub const MAX_TERMINATION_DELAY: u32 = 60;
    /// Maximum disk I/O throughput in MB/s when safe mode is enabled
    pub const MAX_DISK_MBPS: u32 = 100;
    /// Maximum file size in MB when safe mode is enabled
    pub const MAX_DISK_FILE_SIZE_MB: u32 = 1024;
}

/// Check if safe mode is enabled via environment variable.
/// Default is true (safe mode ON) unless explicitly set to "false".
pub fn is_safe_mode_enabled() -> bool {
    static SAFE_MODE: OnceLock<bool> = OnceLock::new();
    *SAFE_MODE.get_or_init(|| {
        std::env::var("STRESSOR_SAFE_MODE")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true) // Default: safe mode enabled
    })
}

/// Operation mode - only one active at a time
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OperationMode {
    /// CPU stress testing mode
    CpuStressor,
    /// Memory stress testing mode
    MemoryStressor,
    /// Network stress testing mode
    NetworkStressor,
    /// Disk I/O stress testing mode
    DiskStressor,
    /// No active stressor
    #[default]
    Idle,
}

/// Load curve profile for stressors.
///
/// All modes cycle: ramp → hold → ramp-down → interval rest → repeat
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CurveMode {
    /// Linear ramp up (growth_rate/sec) → hold at max → linear ramp down → rest at start
    #[default]
    Linear,
    /// Instant jump to max → hold for peak duration → drop to start → rest for interval
    Burst,
    /// Sigmoid (S-curve) ramp up → hold at max → sigmoid ramp down → rest at start
    SCurve,
}

/// CPU stressor configuration.
///
/// Cycle: ramp up → hold at max (until midpoint) → ramp down → rest at start_value for interval.
/// Cycles repeat until stopped via API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct CpuConfig {
    /// Curve type: linear, burst, or s-curve
    #[schema(default = "linear")]
    pub mode: CurveMode,

    /// Maximum CPU load in milli-cores (e.g., 2000 = 2 full cores). Default: 1000
    #[schema(default = 1000, minimum = 1)]
    pub max_value: u32,

    /// Starting/minimum CPU load in milli-cores. Default: 100
    #[schema(default = 100, minimum = 0)]
    pub start_value: u32,

    /// Load change rate in milli-cores per second (Linear/S-Curve). Default: 100
    #[schema(default = 100, minimum = 1)]
    pub growth_rate: u32,

    /// Midpoint time in milliseconds. Peak is reached at midpoint, then ramp down begins.
    /// For Burst: duration at max before dropping. Default: 30000 (30s)
    #[schema(default = 30000, minimum = 1000)]
    pub midpoint_ms: u32,

    /// Rest interval at start_value between cycles, in seconds. Default: 10
    #[schema(default = 10, minimum = 0)]
    pub interval: u64,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            mode: CurveMode::Linear,
            max_value: 1000,
            start_value: 100,
            growth_rate: 100,
            midpoint_ms: 30000,
            interval: 10,
        }
    }
}

impl CpuConfig {
    /// Validate configuration values.
    /// When safe mode is enabled, enforces maximum limits to prevent cluster damage.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_value == 0 {
            return Err("max_value must be greater than 0".to_string());
        }
        if self.start_value > self.max_value {
            return Err("start_value cannot exceed max_value".to_string());
        }
        if self.midpoint_ms < 1000 {
            return Err("midpoint_ms must be at least 1000ms".to_string());
        }
        if self.growth_rate == 0 {
            return Err("growth_rate must be greater than 0".to_string());
        }

        // Safe mode limits
        if is_safe_mode_enabled() && self.max_value > safe_mode::MAX_CPU_MILLICORES {
            return Err(format!(
                "max_value {} exceeds safe mode limit of {} millicores. Set STRESSOR_SAFE_MODE=false to disable limits.",
                self.max_value, safe_mode::MAX_CPU_MILLICORES
            ));
        }

        Ok(())
    }

    /// Calculate ramp duration based on growth_rate
    pub fn ramp_duration_ms(&self) -> u64 {
        let delta = (self.max_value - self.start_value) as u64;
        (delta * 1000) / self.growth_rate as u64
    }

    /// Calculate total cycle duration (ramp up + hold + ramp down + interval)
    pub fn cycle_duration_ms(&self) -> u64 {
        // midpoint_ms is when ramp down starts
        // Total active = 2 * midpoint_ms (symmetric)
        // Plus interval rest
        (self.midpoint_ms as u64 * 2) + (self.interval * 1000)
    }
}

/// Memory stressor configuration.
///
/// Follows the same curve behavior as CPU: ramp up → hold at max → ramp down → rest.
/// Memory allocation grows/shrinks following the selected curve mode.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct MemoryConfig {
    /// Curve type: linear, burst, or s-curve. Default: linear
    #[schema(default = "linear")]
    pub mode: CurveMode,

    /// Target/maximum memory allocation in MB. Default: 256
    #[schema(default = 256, minimum = 1)]
    pub target_mb: u32,

    /// Starting/minimum memory allocation in MB. Default: 0
    #[schema(default = 0, minimum = 0)]
    pub start_mb: u32,

    /// Allocation change rate in MB per second (Linear/S-Curve). Default: 10
    #[schema(default = 10, minimum = 1)]
    pub growth_rate: u32,

    /// Midpoint time in milliseconds. Peak allocation at midpoint, then deallocation begins.
    /// For Burst: duration at max before releasing. Default: 30000 (30s)
    #[schema(default = 30000, minimum = 1000)]
    pub midpoint_ms: u32,

    /// Rest interval at start_mb between cycles, in seconds. Default: 10
    #[schema(default = 10, minimum = 0)]
    pub interval: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            mode: CurveMode::Linear,
            target_mb: 256,
            start_mb: 0,
            growth_rate: 10,
            midpoint_ms: 30000,
            interval: 10,
        }
    }
}

impl MemoryConfig {
    /// Validate configuration values.
    /// When safe mode is enabled, enforces maximum limits to prevent cluster damage.
    pub fn validate(&self) -> Result<(), String> {
        if self.target_mb == 0 {
            return Err("target_mb must be greater than 0".to_string());
        }
        if self.start_mb >= self.target_mb {
            return Err("start_mb must be less than target_mb".to_string());
        }
        if self.midpoint_ms < 1000 {
            return Err("midpoint_ms must be at least 1000ms".to_string());
        }
        if self.growth_rate == 0 {
            return Err("growth_rate must be greater than 0".to_string());
        }

        // Safe mode limits
        if is_safe_mode_enabled() && self.target_mb > safe_mode::MAX_MEMORY_MB {
            return Err(format!(
                "target_mb {} exceeds safe mode limit of {} MB. Set STRESSOR_SAFE_MODE=false to disable limits.",
                self.target_mb, safe_mode::MAX_MEMORY_MB
            ));
        }

        Ok(())
    }

    /// Calculate ramp duration based on growth_rate
    pub fn ramp_duration_ms(&self) -> u64 {
        let delta = (self.target_mb - self.start_mb) as u64;
        (delta * 1000) / self.growth_rate as u64
    }
}

/// Network stressor configuration.
///
/// Network stressor floods connections to target endpoint.
/// Uses midpoint for active duration, then rests for interval.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct NetworkConfig {
    /// Target endpoint URL. Default: http://localhost:8080/health
    #[schema(default = "http://localhost:8080/health")]
    pub endpoint: String,

    /// Protocol: http, tcp, udp. Default: http
    #[schema(default = "http")]
    pub protocol: String,

    /// Number of concurrent connections. Default: 10
    #[schema(default = 10, minimum = 1)]
    pub connections: u32,

    /// Active duration in milliseconds (floods for this duration). Default: 30000 (30s)
    #[schema(default = 30000, minimum = 1000)]
    pub midpoint_ms: u32,

    /// Rest interval between cycles, in seconds. Default: 10
    #[schema(default = 10, minimum = 0)]
    pub interval: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            endpoint: String::from("http://localhost:8080/health"),
            protocol: String::from("http"),
            connections: 10,
            midpoint_ms: 30000,
            interval: 10,
        }
    }
}

impl NetworkConfig {
    /// Validate configuration values.
    /// When safe mode is enabled, enforces maximum limits to prevent cluster damage.
    pub fn validate(&self) -> Result<(), String> {
        if self.endpoint.is_empty() {
            return Err("endpoint cannot be empty".to_string());
        }
        if self.connections == 0 {
            return Err("connections must be greater than 0".to_string());
        }
        if !["http", "tcp", "udp"].contains(&self.protocol.as_str()) {
            return Err("protocol must be http, tcp, or udp".to_string());
        }
        if self.midpoint_ms < 1000 {
            return Err("midpoint_ms must be at least 1000ms".to_string());
        }

        // Safe mode limits
        if is_safe_mode_enabled() && self.connections > safe_mode::MAX_NETWORK_CONNECTIONS {
            return Err(format!(
                "connections {} exceeds safe mode limit of {}. Set STRESSOR_SAFE_MODE=false to disable limits.",
                self.connections, safe_mode::MAX_NETWORK_CONNECTIONS
            ));
        }

        Ok(())
    }
}

/// Chaos/lifecycle simulation configuration.
///
/// Controls probe failure simulation and graceful shutdown behavior
/// for testing Kubernetes lifecycle handling.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct ChaosConfig {
    /// When true, /health endpoint returns 503 to simulate liveness probe failure.
    /// Kubernetes will restart the pod based on livenessProbe configuration.
    #[schema(default = false)]
    pub fail_liveness: bool,

    /// When true, /ready endpoint returns 503 to simulate readiness probe failure.
    /// Kubernetes will remove the pod from service endpoints.
    #[schema(default = false)]
    pub fail_readiness: bool,

    /// Delay in seconds before honoring SIGTERM shutdown signal.
    /// Simulates "zombie" pods that don't terminate promptly.
    /// Use to test terminationGracePeriodSeconds configuration.
    /// 0 = immediate shutdown (default behavior).
    #[schema(default = 0, minimum = 0, maximum = 300)]
    pub termination_delay_seconds: u32,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            fail_liveness: false,
            fail_readiness: false,
            termination_delay_seconds: 0,
        }
    }
}

impl ChaosConfig {
    /// Validate chaos configuration values.
    /// When safe mode is enabled, enforces maximum limits to prevent cluster damage.
    pub fn validate(&self) -> Result<(), String> {
        if self.termination_delay_seconds > 300 {
            return Err("termination_delay_seconds cannot exceed 300 seconds".to_string());
        }

        // Safe mode limits
        if is_safe_mode_enabled() && self.termination_delay_seconds > safe_mode::MAX_TERMINATION_DELAY {
            return Err(format!(
                "termination_delay_seconds {} exceeds safe mode limit of {} seconds. Set STRESSOR_SAFE_MODE=false to disable limits.",
                self.termination_delay_seconds, safe_mode::MAX_TERMINATION_DELAY
            ));
        }

        Ok(())
    }
}

/// I/O pattern for disk stressor
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IoPattern {
    /// Sequential I/O (linear read/write through file)
    #[default]
    Sequential,
    /// Random I/O (random seek positions within file)
    Random,
}

/// Disk I/O stressor configuration.
///
/// Generates controlled read/write load on specified paths.
/// Supports multiple volumes for storage class comparison.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct DiskConfig {
    /// Curve type: linear, burst, or s-curve. Default: linear
    #[schema(default = "linear")]
    pub mode: CurveMode,

    /// Target I/O throughput in MB/s. Default: 50
    #[schema(default = 50, minimum = 1)]
    pub target_mbps: u32,

    /// Starting I/O throughput in MB/s. Default: 10
    #[schema(default = 10, minimum = 1)]
    pub start_mbps: u32,

    /// I/O pattern: sequential or random. Default: sequential
    #[schema(default = "sequential")]
    pub pattern: IoPattern,

    /// Read/write ratio (0.0 = all writes, 1.0 = all reads). Default: 0.5
    #[schema(default = 0.5, minimum = 0.0, maximum = 1.0)]
    pub read_ratio: f32,

    /// Block size for I/O operations in KB. Default: 4 (4KB)
    #[schema(default = 4, minimum = 1, maximum = 1024)]
    pub block_size_kb: u32,

    /// Working directory for temp files. Default: /tmp/k8s-stressor
    /// Can be overridden to target specific volumes (PVCs, different storage classes)
    #[schema(default = "/tmp/k8s-stressor")]
    pub work_dir: String,

    /// Additional work directories for multi-volume testing.
    /// Each path will have its own temp file and I/O operations.
    #[schema(default = "[]")]
    pub additional_paths: Vec<String>,

    /// Maximum file size in MB. Default: 512
    #[schema(default = 512, minimum = 1)]
    pub max_file_size_mb: u32,

    /// Midpoint time in milliseconds. Default: 30000 (30s)
    #[schema(default = 30000, minimum = 1000)]
    pub midpoint_ms: u32,

    /// Rest interval between cycles, in seconds. Default: 10
    #[schema(default = 10, minimum = 0)]
    pub interval: u64,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            mode: CurveMode::Linear,
            target_mbps: 50,
            start_mbps: 10,
            pattern: IoPattern::Sequential,
            read_ratio: 0.5,
            block_size_kb: 4,
            work_dir: std::env::var("STRESSOR_DISK_WORK_DIR")
                .unwrap_or_else(|_| "/tmp/k8s-stressor".to_string()),
            additional_paths: Vec::new(),
            max_file_size_mb: 512,
            midpoint_ms: 30000,
            interval: 10,
        }
    }
}

impl DiskConfig {
    /// Validate configuration values.
    /// When safe mode is enabled, enforces maximum limits to prevent cluster damage.
    pub fn validate(&self) -> Result<(), String> {
        if self.target_mbps == 0 {
            return Err("target_mbps must be greater than 0".to_string());
        }
        if self.start_mbps > self.target_mbps {
            return Err("start_mbps cannot exceed target_mbps".to_string());
        }
        if self.read_ratio < 0.0 || self.read_ratio > 1.0 {
            return Err("read_ratio must be between 0.0 and 1.0".to_string());
        }
        if self.block_size_kb == 0 || self.block_size_kb > 1024 {
            return Err("block_size_kb must be between 1 and 1024".to_string());
        }
        if self.work_dir.is_empty() {
            return Err("work_dir cannot be empty".to_string());
        }
        if self.max_file_size_mb == 0 {
            return Err("max_file_size_mb must be greater than 0".to_string());
        }
        if self.midpoint_ms < 1000 {
            return Err("midpoint_ms must be at least 1000ms".to_string());
        }

        // Safe mode limits
        if is_safe_mode_enabled() {
            if self.target_mbps > safe_mode::MAX_DISK_MBPS {
                return Err(format!(
                    "target_mbps {} exceeds safe mode limit of {} MB/s. Set STRESSOR_SAFE_MODE=false to disable limits.",
                    self.target_mbps, safe_mode::MAX_DISK_MBPS
                ));
            }
            if self.max_file_size_mb > safe_mode::MAX_DISK_FILE_SIZE_MB {
                return Err(format!(
                    "max_file_size_mb {} exceeds safe mode limit of {} MB. Set STRESSOR_SAFE_MODE=false to disable limits.",
                    self.max_file_size_mb, safe_mode::MAX_DISK_FILE_SIZE_MB
                ));
            }
        }

        Ok(())
    }

    /// Get all paths (work_dir + additional_paths)
    pub fn all_paths(&self) -> Vec<String> {
        let mut paths = vec![self.work_dir.clone()];
        paths.extend(self.additional_paths.clone());
        paths
    }

    /// Calculate ramp duration based on growth rate (MB/s per second)
    pub fn ramp_duration_ms(&self) -> u64 {
        let delta = (self.target_mbps - self.start_mbps) as u64;
        // Assume 10 MB/s growth rate per second
        let growth_rate = 10;
        (delta * 1000) / growth_rate
    }

    /// Calculate total cycle duration
    pub fn cycle_duration_ms(&self) -> u64 {
        (self.midpoint_ms as u64 * 2) + (self.interval * 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_config_default() {
        let config = CpuConfig::default();
        assert_eq!(config.mode, CurveMode::Linear);
        assert_eq!(config.max_value, 1000);
        assert_eq!(config.start_value, 100);
        assert_eq!(config.growth_rate, 100);
        assert_eq!(config.midpoint_ms, 30000);
        assert_eq!(config.interval, 10);
    }

    #[test]
    fn test_cpu_config_validation_valid() {
        let config = CpuConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_cpu_config_validation_invalid_max() {
        let config = CpuConfig {
            max_value: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cpu_config_validation_start_exceeds_max() {
        let config = CpuConfig {
            start_value: 2000,
            max_value: 1000,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cpu_config_ramp_duration() {
        let config = CpuConfig {
            max_value: 1000,
            start_value: 100,
            growth_rate: 100, // 100 milli-cores per second
            ..Default::default()
        };
        // Delta = 900, rate = 100/s, so ramp takes 9 seconds = 9000ms
        assert_eq!(config.ramp_duration_ms(), 9000);
    }

    #[test]
    fn test_cpu_config_cycle_duration() {
        let config = CpuConfig {
            midpoint_ms: 30000,
            interval: 10,
            ..Default::default()
        };
        // Total = 2 * 30000ms + 10 * 1000ms = 60000 + 10000 = 70000ms
        assert_eq!(config.cycle_duration_ms(), 70000);
    }

    #[test]
    fn test_memory_config_default() {
        let config = MemoryConfig::default();
        assert_eq!(config.mode, CurveMode::Linear);
        assert_eq!(config.target_mb, 256);
        assert_eq!(config.start_mb, 0);
        assert_eq!(config.growth_rate, 10);
        assert_eq!(config.midpoint_ms, 30000);
        assert_eq!(config.interval, 10);
    }

    #[test]
    fn test_memory_config_validation() {
        let valid = MemoryConfig::default();
        assert!(valid.validate().is_ok());

        let invalid = MemoryConfig {
            target_mb: 0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_memory_config_ramp_duration() {
        let config = MemoryConfig {
            target_mb: 256,
            start_mb: 0,
            growth_rate: 10, // 10 MB per second
            ..Default::default()
        };
        // Delta = 256MB, rate = 10MB/s, so ramp takes 25.6 seconds = 25600ms
        assert_eq!(config.ramp_duration_ms(), 25600);
    }

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8080/health");
        assert_eq!(config.protocol, "http");
        assert_eq!(config.connections, 10);
        assert_eq!(config.midpoint_ms, 30000);
        assert_eq!(config.interval, 10);
    }

    #[test]
    fn test_network_config_validation() {
        let valid = NetworkConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_protocol = NetworkConfig {
            protocol: "ftp".to_string(),
            ..Default::default()
        };
        assert!(invalid_protocol.validate().is_err());
    }

    #[test]
    fn test_operation_mode_serialization() {
        let mode = OperationMode::CpuStressor;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"cpu-stressor\"");

        let deserialized: OperationMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OperationMode::CpuStressor);
    }

    #[test]
    fn test_curve_mode_serialization() {
        let mode = CurveMode::SCurve;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"s-curve\"");
    }

    #[test]
    fn test_chaos_config_default() {
        let config = ChaosConfig::default();
        assert!(!config.fail_liveness);
        assert!(!config.fail_readiness);
        assert_eq!(config.termination_delay_seconds, 0);
    }

    #[test]
    fn test_chaos_config_validation() {
        let valid = ChaosConfig::default();
        assert!(valid.validate().is_ok());

        let invalid = ChaosConfig {
            termination_delay_seconds: 301,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_chaos_config_serialization() {
        let config = ChaosConfig {
            fail_liveness: true,
            fail_readiness: false,
            termination_delay_seconds: 30,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"fail_liveness\":true"));
        assert!(json.contains("\"termination_delay_seconds\":30"));

        let deserialized: ChaosConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }

    #[test]
    fn test_safe_mode_limits() {
        // Note: These tests verify the safe_mode module constants are defined correctly
        assert_eq!(safe_mode::MAX_CPU_MILLICORES, 2000);
        assert_eq!(safe_mode::MAX_MEMORY_MB, 2048);
        assert_eq!(safe_mode::MAX_NETWORK_CONNECTIONS, 100);
        assert_eq!(safe_mode::MAX_TERMINATION_DELAY, 60);
        assert_eq!(safe_mode::MAX_DISK_MBPS, 100);
        assert_eq!(safe_mode::MAX_DISK_FILE_SIZE_MB, 1024);
    }

    #[test]
    fn test_disk_config_default() {
        let config = DiskConfig::default();
        assert_eq!(config.mode, CurveMode::Linear);
        assert_eq!(config.target_mbps, 50);
        assert_eq!(config.start_mbps, 10);
        assert_eq!(config.pattern, IoPattern::Sequential);
        assert_eq!(config.read_ratio, 0.5);
        assert_eq!(config.block_size_kb, 4);
        assert_eq!(config.max_file_size_mb, 512);
        assert_eq!(config.midpoint_ms, 30000);
        assert_eq!(config.interval, 10);
    }

    #[test]
    fn test_disk_config_validation() {
        let valid = DiskConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_target = DiskConfig {
            target_mbps: 0,
            ..Default::default()
        };
        assert!(invalid_target.validate().is_err());

        let invalid_start = DiskConfig {
            start_mbps: 100,
            target_mbps: 50,
            ..Default::default()
        };
        assert!(invalid_start.validate().is_err());

        let invalid_ratio = DiskConfig {
            read_ratio: 1.5,
            ..Default::default()
        };
        assert!(invalid_ratio.validate().is_err());
    }

    #[test]
    fn test_disk_config_all_paths() {
        let config = DiskConfig {
            work_dir: "/tmp/test".to_string(),
            additional_paths: vec!["/mnt/ssd".to_string(), "/mnt/hdd".to_string()],
            ..Default::default()
        };
        let paths = config.all_paths();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], "/tmp/test");
        assert_eq!(paths[1], "/mnt/ssd");
        assert_eq!(paths[2], "/mnt/hdd");
    }

    #[test]
    fn test_io_pattern_serialization() {
        let pattern = IoPattern::Random;
        let json = serde_json::to_string(&pattern).unwrap();
        assert_eq!(json, "\"random\"");

        let deserialized: IoPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, IoPattern::Random);
    }

    #[test]
    fn test_operation_mode_disk_stressor() {
        let mode = OperationMode::DiskStressor;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"disk-stressor\"");

        let deserialized: OperationMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OperationMode::DiskStressor);
    }
}
