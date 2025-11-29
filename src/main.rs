// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: 2024-11-28
// License: MIT

use std::net::SocketAddr;

use axum::{routing::{get, post}, Router};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use k8s_stressor::api::handlers::{self, StatusResponse};
use k8s_stressor::config::{CpuConfig, CurveMode, MemoryConfig, NetworkConfig, OperationMode};
use k8s_stressor::state::create_shared_state;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(OpenApi)]
#[openapi(
    info(
        title = "k8s-stressor API",
        version = "0.1.0",
        description = "Deterministic resource consumption for Kubernetes reliability testing",
        license(name = "MIT"),
        contact(name = "Luis Amaral", url = "https://github.com/luiscamaral")
    ),
    paths(
        handlers::health,
        handlers::get_status,
        handlers::get_metrics,
        handlers::set_mode,
        handlers::set_cpu_config,
        handlers::set_memory_config,
        handlers::set_network_config,
        handlers::stop_all,
    ),
    components(schemas(
        StatusResponse,
        OperationMode,
        CurveMode,
        CpuConfig,
        MemoryConfig,
        NetworkConfig,
    )),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Status", description = "Status and info endpoints"),
        (name = "Metrics", description = "Prometheus metrics"),
        (name = "Control", description = "Stressor control endpoints"),
        (name = "Configuration", description = "Stressor configuration endpoints"),
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

    // Build router with Swagger UI
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::health))
        .route("/status", get(handlers::get_status))
        .route("/metrics", get(handlers::get_metrics))
        .route("/mode", post(handlers::set_mode))
        .route("/cpu", post(handlers::set_cpu_config))
        .route("/memory", post(handlers::set_memory_config))
        .route("/network", post(handlers::set_network_config))
        .route("/stop", post(handlers::stop_all))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("k8s-stressor v{} starting on {}", VERSION, addr);
    tracing::info!("Swagger UI available at http://localhost:8080/swagger-ui/");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
