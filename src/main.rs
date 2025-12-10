// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{get, put},
    Router,
};
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use k8s_stressor::api::handlers::{self, AppContext, HealthResponse, StatusResponse};
use k8s_stressor::config::{ChaosConfig, CpuConfig, CurveMode, DiskConfig, IoPattern, MemoryConfig, NetworkConfig, OperationMode};
use k8s_stressor::orchestrator::Orchestrator;
use k8s_stressor::state::create_shared_state;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(OpenApi)]
#[openapi(
    info(
        title = "k8s-stressor API",
        version = "0.4.0",
        description = "Deterministic resource consumption for Kubernetes reliability testing",
        license(name = "MIT"),
        contact(name = "Luis Amaral", url = "https://github.com/luiscamaral")
    ),
    paths(
        handlers::health,
        handlers::ready,
        handlers::get_status,
        handlers::get_metrics,
        handlers::get_mode,
        handlers::set_mode,
        handlers::get_cpu_config,
        handlers::set_cpu_config,
        handlers::get_memory_config,
        handlers::set_memory_config,
        handlers::get_network_config,
        handlers::set_network_config,
        handlers::get_disk_config,
        handlers::set_disk_config,
        handlers::get_chaos_config,
        handlers::set_chaos_config,
        handlers::stop_all,
    ),
    components(schemas(
        StatusResponse,
        HealthResponse,
        OperationMode,
        CurveMode,
        CpuConfig,
        MemoryConfig,
        NetworkConfig,
        DiskConfig,
        IoPattern,
        ChaosConfig,
    )),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Status", description = "Status and info endpoints"),
        (name = "Metrics", description = "Prometheus metrics"),
        (name = "Control", description = "Stressor control endpoints"),
        (name = "Configuration", description = "Stressor configuration endpoints"),
        (name = "Chaos", description = "Chaos/lifecycle simulation endpoints"),
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create shared state
    let state = create_shared_state();

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start orchestrator
    let orchestrator = Orchestrator::new(state.clone(), shutdown_rx);
    let orchestrator_metrics = orchestrator.metrics();

    let orchestrator_handle = tokio::spawn(async move {
        orchestrator.run().await;
    });

    // Create app context with state and metrics
    let app_context = Arc::new(AppContext {
        state: state.clone(),
        metrics: orchestrator_metrics,
    });

    // Build router with Swagger UI
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Health endpoints
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        // Status and metrics
        .route("/status", get(handlers::get_status))
        .route("/metrics", get(handlers::get_metrics))
        // Mode control
        .route("/mode", get(handlers::get_mode).put(handlers::set_mode))
        // Configuration endpoints
        .route(
            "/config/cpu",
            get(handlers::get_cpu_config).put(handlers::set_cpu_config),
        )
        .route(
            "/config/memory",
            get(handlers::get_memory_config).put(handlers::set_memory_config),
        )
        .route(
            "/config/network",
            get(handlers::get_network_config).put(handlers::set_network_config),
        )
        .route(
            "/config/disk",
            get(handlers::get_disk_config).put(handlers::set_disk_config),
        )
        .route(
            "/config/chaos",
            get(handlers::get_chaos_config).put(handlers::set_chaos_config),
        )
        // Stop control
        .route("/stop", put(handlers::stop_all))
        .layer(TraceLayer::new_for_http())
        .with_state(app_context);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("k8s-stressor v{} starting on {}", VERSION, addr);
    tracing::info!("Swagger UI available at http://localhost:8080/swagger-ui/");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    // Handle shutdown
    let server = axum::serve(listener, app);

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal (SIGINT)");
            handle_graceful_shutdown(&state, &shutdown_tx).await;
        }
    }

    // Wait for orchestrator to finish
    let _ = orchestrator_handle.await;
    tracing::info!("Shutdown complete");
}

/// Handle graceful shutdown with optional termination delay (zombie mode)
async fn handle_graceful_shutdown(
    state: &k8s_stressor::state::SharedState,
    shutdown_tx: &watch::Sender<bool>,
) {
    let delay_seconds = {
        let s = state.read().await;
        s.chaos_config.termination_delay_seconds
    };

    if delay_seconds > 0 {
        tracing::warn!(
            "Chaos mode: delaying shutdown for {} seconds (zombie mode)",
            delay_seconds
        );
        tokio::time::sleep(std::time::Duration::from_secs(delay_seconds as u64)).await;
        tracing::info!("Termination delay complete, proceeding with shutdown");
    }

    let _ = shutdown_tx.send(true);
}
