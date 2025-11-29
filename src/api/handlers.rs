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
use utoipa::ToSchema;

use crate::config::{CpuConfig, MemoryConfig, NetworkConfig, OperationMode};
use crate::error::AppError;
use crate::state::SharedState;

/// Response for status endpoint
#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    /// Current operation mode
    pub mode: OperationMode,
    /// Configuration version counter
    pub config_version: u64,
    /// CPU stressor configuration
    pub cpu_config: CpuConfig,
    /// Memory stressor configuration
    pub memory_config: MemoryConfig,
    /// Network stressor configuration
    pub network_config: NetworkConfig,
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = String)
    )
)]
pub async fn health() -> &'static str {
    "OK"
}

/// Get current status and configuration
#[utoipa::path(
    get,
    path = "/status",
    tag = "Status",
    responses(
        (status = 200, description = "Current stressor status", body = StatusResponse)
    )
)]
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

/// Get Prometheus metrics
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Metrics",
    responses(
        (status = 200, description = "Prometheus format metrics", content_type = "text/plain")
    )
)]
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

/// Set operation mode
#[utoipa::path(
    post,
    path = "/mode",
    tag = "Control",
    request_body = OperationMode,
    responses(
        (status = 200, description = "Mode updated successfully")
    )
)]
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

/// Configure CPU stressor
#[utoipa::path(
    post,
    path = "/cpu",
    tag = "Configuration",
    request_body = CpuConfig,
    responses(
        (status = 200, description = "CPU configuration updated"),
        (status = 400, description = "Invalid configuration")
    )
)]
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

/// Configure memory stressor
#[utoipa::path(
    post,
    path = "/memory",
    tag = "Configuration",
    request_body = MemoryConfig,
    responses(
        (status = 200, description = "Memory configuration updated"),
        (status = 400, description = "Invalid configuration")
    )
)]
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

/// Configure network stressor
#[utoipa::path(
    post,
    path = "/network",
    tag = "Configuration",
    request_body = NetworkConfig,
    responses(
        (status = 200, description = "Network configuration updated"),
        (status = 400, description = "Invalid configuration")
    )
)]
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

/// Stop all stressors and reset to idle
#[utoipa::path(
    post,
    path = "/stop",
    tag = "Control",
    responses(
        (status = 200, description = "All stressors stopped")
    )
)]
pub async fn stop_all(State(state): State<SharedState>) -> impl IntoResponse {
    let mut s = state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    StatusCode::OK
}
