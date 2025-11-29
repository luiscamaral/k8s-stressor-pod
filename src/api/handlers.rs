// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::sync::atomic::Ordering;
use std::sync::Arc;

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
use crate::orchestrator::OrchestratorMetrics;
use crate::state::SharedState;

/// Application context with state and metrics
#[derive(Clone)]
pub struct AppContext {
    pub state: SharedState,
    pub metrics: Arc<OrchestratorMetrics>,
}

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
pub async fn get_status(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
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
pub async fn get_metrics(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    let mode_num = match s.current_mode {
        OperationMode::Idle => 0,
        OperationMode::CpuStressor => 1,
        OperationMode::MemoryStressor => 2,
        OperationMode::NetworkStressor => 3,
    };

    // Get metrics from orchestrator
    let cpu_target = ctx.metrics.cpu_target_millicores.load(Ordering::Relaxed);
    let cpu_threads = ctx.metrics.cpu_active_threads.load(Ordering::Relaxed);
    let mem_target = ctx.metrics.memory_target_bytes.load(Ordering::Relaxed);
    let mem_allocated = ctx.metrics.memory_allocated_bytes.load(Ordering::Relaxed);
    let net_connections = ctx.metrics.network_active_connections.load(Ordering::Relaxed);
    let net_requests = ctx.metrics.network_requests_total.load(Ordering::Relaxed);
    let net_errors = ctx.metrics.network_errors_total.load(Ordering::Relaxed);

    let body = format!(
        "# HELP stressor_mode Current operation mode (0=idle, 1=cpu, 2=memory, 3=network)\n\
         # TYPE stressor_mode gauge\n\
         stressor_mode {}\n\
         # HELP stressor_config_version Configuration version counter\n\
         # TYPE stressor_config_version counter\n\
         stressor_config_version {}\n\
         # HELP stressor_is_active Whether a stressor is currently running\n\
         # TYPE stressor_is_active gauge\n\
         stressor_is_active {}\n\
         # HELP stressor_cpu_target_millicores Target CPU load in millicores\n\
         # TYPE stressor_cpu_target_millicores gauge\n\
         stressor_cpu_target_millicores {}\n\
         # HELP stressor_cpu_active_threads Number of active CPU worker threads\n\
         # TYPE stressor_cpu_active_threads gauge\n\
         stressor_cpu_active_threads {}\n\
         # HELP stressor_memory_target_bytes Target memory allocation in bytes\n\
         # TYPE stressor_memory_target_bytes gauge\n\
         stressor_memory_target_bytes {}\n\
         # HELP stressor_memory_allocated_bytes Current memory allocation in bytes\n\
         # TYPE stressor_memory_allocated_bytes gauge\n\
         stressor_memory_allocated_bytes {}\n\
         # HELP stressor_network_active_connections Number of active network connections\n\
         # TYPE stressor_network_active_connections gauge\n\
         stressor_network_active_connections {}\n\
         # HELP stressor_network_requests_total Total network requests made\n\
         # TYPE stressor_network_requests_total counter\n\
         stressor_network_requests_total {}\n\
         # HELP stressor_network_errors_total Total network errors\n\
         # TYPE stressor_network_errors_total counter\n\
         stressor_network_errors_total {}\n",
        mode_num,
        s.config_version,
        if s.current_mode == OperationMode::Idle { 0 } else { 1 },
        cpu_target,
        cpu_threads,
        mem_target,
        mem_allocated,
        net_connections,
        net_requests,
        net_errors,
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
    State(ctx): State<Arc<AppContext>>,
    Json(mode): Json<OperationMode>,
) -> impl IntoResponse {
    let mut s = ctx.state.write().await;
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
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<CpuConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
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
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<MemoryConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
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
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<NetworkConfig>,
) -> Result<StatusCode, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
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
pub async fn stop_all(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let mut s = ctx.state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    StatusCode::OK
}
