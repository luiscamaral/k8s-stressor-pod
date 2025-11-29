// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};
use crate::error::AppError;
use crate::state::SharedState;

/// Response for status endpoint
#[derive(Serialize)]
pub struct StatusResponse {
    pub mode: OperationMode,
    pub config_version: u64,
    pub cpu_config: CpuConfig,
    pub memory_config: MemoryConfig,
    pub network_config: NetworkConfig,
}

/// GET /health
pub async fn health() -> &'static str {
    "OK"
}

/// GET /status
pub async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    Json(StatusResponse {
        mode: s.current_mode.clone(),
        config_version: s.config_version,
        cpu_config: s.cpu_config.clone(),
        memory_config: s.memory_config.clone(),
        network_config: s.network_config.clone(),
    })
}

/// GET /metrics (placeholder for Phase 1)
pub async fn get_metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    let mode_num = match s.current_mode {
        OperationMode::Idle => 0,
        OperationMode::CpuStressor => 1,
        OperationMode::MemoryStressor => 2,
        OperationMode::NetworkStressor => 3,
    };

    let body = format!(
        "# HELP stressor_mode Current operation mode (0=idle, 1=cpu, 2=memory, 3=network)\n\
         # TYPE stressor_mode gauge\n\
         stressor_mode {}\n\
         # HELP stressor_config_version Configuration version counter\n\
         # TYPE stressor_config_version counter\n\
         stressor_config_version {}\n\
         # HELP stressor_is_active Whether a stressor is currently running\n\
         # TYPE stressor_is_active gauge\n\
         stressor_is_active {}\n",
        mode_num,
        s.config_version,
        if s.current_mode == OperationMode::Idle { 0 } else { 1 }
    );

    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

/// POST /mode
pub async fn set_mode(
    State(state): State<SharedState>,
    Json(mode): Json<OperationMode>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Mode change: {:?} -> {:?}", s.current_mode, mode);
    s.current_mode = mode;
    s.bump_version();
    StatusCode::OK
}

/// POST /cpu
pub async fn set_cpu_config(
    State(state): State<SharedState>,
    Json(config): Json<CpuConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("CPU config updated: {:?}", config);
    s.cpu_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /memory
pub async fn set_memory_config(
    State(state): State<SharedState>,
    Json(config): Json<MemoryConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("Memory config updated: {:?}", config);
    s.memory_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /network
pub async fn set_network_config(
    State(state): State<SharedState>,
    Json(config): Json<NetworkConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = state.write().await;
    tracing::info!("Network config updated: {:?}", config);
    s.network_config = config;
    s.bump_version();
    Ok(StatusCode::OK)
}

/// POST /stop
pub async fn stop_all(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    StatusCode::OK
}
