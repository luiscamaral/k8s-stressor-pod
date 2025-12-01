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

/// Response for status endpoint - runtime status only
#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    /// Current operation mode
    pub mode: OperationMode,
    /// Configuration version counter
    pub config_version: u64,
    /// Whether a stressor is currently active
    pub is_active: bool,
}

/// Response for health/ready endpoints
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Health status
    pub status: String,
    /// Application version
    pub version: String,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Liveness probe - indicates the service is running
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse)
    )
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: VERSION.to_string(),
    })
}

/// Readiness probe - indicates the service is ready to accept traffic
#[utoipa::path(
    get,
    path = "/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready", body = HealthResponse),
        (status = 503, description = "Service is not ready")
    )
)]
pub async fn ready(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    // Check if we can acquire the state lock (service is responsive)
    let _state = ctx.state.read().await;
    Json(HealthResponse {
        status: "ready".to_string(),
        version: VERSION.to_string(),
    })
}

/// Get current runtime status
#[utoipa::path(
    get,
    path = "/status",
    tag = "Status",
    responses(
        (status = 200, description = "Current runtime status", body = StatusResponse)
    )
)]
pub async fn get_status(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    Json(StatusResponse {
        mode: s.current_mode.clone(),
        config_version: s.config_version,
        is_active: s.current_mode != OperationMode::Idle,
    })
}

/// Get current operation mode
#[utoipa::path(
    get,
    path = "/mode",
    tag = "Control",
    responses(
        (status = 200, description = "Current operation mode", body = OperationMode)
    )
)]
pub async fn get_mode(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    Json(s.current_mode.clone())
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
    put,
    path = "/mode",
    tag = "Control",
    request_body = OperationMode,
    responses(
        (status = 200, description = "Mode updated successfully", body = OperationMode)
    )
)]
pub async fn set_mode(
    State(ctx): State<Arc<AppContext>>,
    Json(mode): Json<OperationMode>,
) -> impl IntoResponse {
    let mut s = ctx.state.write().await;
    tracing::info!("Mode change: {:?} -> {:?}", s.current_mode, mode);
    s.current_mode = mode.clone();
    s.bump_version();
    Json(mode)
}

/// Get CPU stressor configuration
#[utoipa::path(
    get,
    path = "/config/cpu",
    tag = "Configuration",
    responses(
        (status = 200, description = "Current CPU configuration", body = CpuConfig)
    )
)]
pub async fn get_cpu_config(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    Json(s.cpu_config.clone())
}

/// Set CPU stressor configuration
#[utoipa::path(
    put,
    path = "/config/cpu",
    tag = "Configuration",
    request_body(content = CpuConfig, description = "CPU stressor settings",
        example = json!({
            "mode": "linear",
            "max_value": 1000,
            "start_value": 100,
            "growth_rate": 100,
            "midpoint_ms": 30000,
            "interval": 10
        })
    ),
    responses(
        (status = 200, description = "CPU configuration updated", body = CpuConfig),
        (status = 400, description = "Invalid configuration")
    )
)]
pub async fn set_cpu_config(
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<CpuConfig>,
) -> Result<Json<CpuConfig>, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
    tracing::info!("CPU config updated: {:?}", config);
    s.cpu_config = config.clone();
    s.bump_version();
    Ok(Json(config))
}

/// Get memory stressor configuration
#[utoipa::path(
    get,
    path = "/config/memory",
    tag = "Configuration",
    responses(
        (status = 200, description = "Current memory configuration", body = MemoryConfig)
    )
)]
pub async fn get_memory_config(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    Json(s.memory_config.clone())
}

/// Set memory stressor configuration
#[utoipa::path(
    put,
    path = "/config/memory",
    tag = "Configuration",
    request_body(content = MemoryConfig, description = "Memory stressor settings",
        example = json!({
            "mode": "linear",
            "target_mb": 256,
            "start_mb": 0,
            "growth_rate": 10,
            "midpoint_ms": 30000,
            "interval": 10
        })
    ),
    responses(
        (status = 200, description = "Memory configuration updated", body = MemoryConfig),
        (status = 400, description = "Invalid configuration")
    )
)]
pub async fn set_memory_config(
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<MemoryConfig>,
) -> Result<Json<MemoryConfig>, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
    tracing::info!("Memory config updated: {:?}", config);
    s.memory_config = config.clone();
    s.bump_version();
    Ok(Json(config))
}

/// Get network stressor configuration
#[utoipa::path(
    get,
    path = "/config/network",
    tag = "Configuration",
    responses(
        (status = 200, description = "Current network configuration", body = NetworkConfig)
    )
)]
pub async fn get_network_config(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let s = ctx.state.read().await;
    Json(s.network_config.clone())
}

/// Set network stressor configuration
#[utoipa::path(
    put,
    path = "/config/network",
    tag = "Configuration",
    request_body(content = NetworkConfig, description = "Network stressor settings",
        example = json!({
            "endpoint": "http://localhost:8080/health",
            "protocol": "http",
            "connections": 10,
            "midpoint_ms": 30000,
            "interval": 10
        })
    ),
    responses(
        (status = 200, description = "Network configuration updated", body = NetworkConfig),
        (status = 400, description = "Invalid configuration")
    )
)]
pub async fn set_network_config(
    State(ctx): State<Arc<AppContext>>,
    Json(config): Json<NetworkConfig>,
) -> Result<Json<NetworkConfig>, AppError> {
    config.validate().map_err(AppError::InvalidConfig)?;

    let mut s = ctx.state.write().await;
    tracing::info!("Network config updated: {:?}", config);
    s.network_config = config.clone();
    s.bump_version();
    Ok(Json(config))
}

/// Stop all stressors and reset to idle
#[utoipa::path(
    put,
    path = "/stop",
    tag = "Control",
    responses(
        (status = 200, description = "All stressors stopped", body = StatusResponse)
    )
)]
pub async fn stop_all(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let mut s = ctx.state.write().await;
    tracing::info!("Stopping all stressors");
    s.current_mode = OperationMode::Idle;
    s.bump_version();
    Json(StatusResponse {
        mode: OperationMode::Idle,
        config_version: s.config_version,
        is_active: false,
    })
}
