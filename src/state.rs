// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};

/// Shared application state
#[derive(Clone, Debug)]
pub struct AppState {
    pub current_mode: OperationMode,
    pub cpu_config: CpuConfig,
    pub memory_config: MemoryConfig,
    pub network_config: NetworkConfig,
    /// Incremented on every config change
    pub config_version: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_mode: OperationMode::Idle,
            cpu_config: CpuConfig::default(),
            memory_config: MemoryConfig::default(),
            network_config: NetworkConfig::default(),
            config_version: 0,
        }
    }
}

impl AppState {
    /// Increment version on state change
    pub fn bump_version(&mut self) {
        self.config_version += 1;
    }
}

/// Thread-safe state wrapper
pub type SharedState = Arc<RwLock<AppState>>;

/// Create new shared state instance
pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.current_mode, OperationMode::Idle);
        assert_eq!(state.config_version, 0);
    }

    #[test]
    fn test_app_state_bump_version() {
        let mut state = AppState::default();
        assert_eq!(state.config_version, 0);
        state.bump_version();
        assert_eq!(state.config_version, 1);
        state.bump_version();
        assert_eq!(state.config_version, 2);
    }

    #[tokio::test]
    async fn test_shared_state_read_write() {
        let state = create_shared_state();

        // Read initial state
        {
            let s = state.read().await;
            assert_eq!(s.current_mode, OperationMode::Idle);
        }

        // Write new mode
        {
            let mut s = state.write().await;
            s.current_mode = OperationMode::CpuStressor;
            s.bump_version();
        }

        // Verify change
        {
            let s = state.read().await;
            assert_eq!(s.current_mode, OperationMode::CpuStressor);
            assert_eq!(s.config_version, 1);
        }
    }
}
