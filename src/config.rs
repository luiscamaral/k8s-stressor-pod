// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    /// No active stressor
    #[default]
    Idle,
}

/// Load curve profile for stressors.
/// 
/// All modes cycle: ramp → hold → ramp-down → interval rest → repeat
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
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
    /// Validate configuration values
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
        Ok(())
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
}
