// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use serde::{Deserialize, Serialize};

/// Operation mode - only one active at a time
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OperationMode {
    CpuStressor,
    MemoryStressor,
    NetworkStressor,
    #[default]
    Idle,
}

/// Load curve profile
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CurveMode {
    #[default]
    Linear,
    Burst,
    SCurve,
}

/// CPU stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CpuConfig {
    /// Curve type: linear, burst, or s-curve
    pub mode: CurveMode,
    /// Maximum CPU in milli-cores (e.g., 2000 = 2 cores)
    pub max_value: u32,
    /// Starting CPU in milli-cores
    pub start_value: u32,
    /// Growth rate per second
    pub growth_rate: u32,
    /// Midpoint for s-curve OR duration for burst (milliseconds)
    pub midpoint_maxpoint: u32,
    /// Total duration in seconds
    pub duration: u64,
    /// Rest interval between cycles in seconds
    pub interval: u64,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            mode: CurveMode::Linear,
            max_value: 1000,
            start_value: 100,
            growth_rate: 10,
            midpoint_maxpoint: 30000,
            duration: 60,
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
        if self.duration == 0 {
            return Err("duration must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Memory stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryConfig {
    /// Target memory allocation in MB
    pub target_mb: u32,
    /// Duration to hold allocation in seconds
    pub duration: u64,
    /// Rest interval between cycles in seconds
    pub interval: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            target_mb: 256,
            duration: 60,
            interval: 30,
        }
    }
}

impl MemoryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_mb == 0 {
            return Err("target_mb must be greater than 0".to_string());
        }
        if self.duration == 0 {
            return Err("duration must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Network stressor configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfig {
    /// Target endpoint URL
    pub endpoint: String,
    /// Protocol: http, tcp, udp
    pub protocol: String,
    /// Number of concurrent connections
    pub connections: u32,
    /// Duration in seconds
    pub duration: u64,
    /// Rest interval in seconds
    pub interval: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            endpoint: String::from("http://localhost:8080/health"),
            protocol: String::from("http"),
            connections: 10,
            duration: 60,
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
