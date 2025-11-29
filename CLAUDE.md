# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

k8s-stressor is a Kubernetes-native reliability testing tool in Rust. It runs inside clusters to generate controlled CPU, memory, and network stress for validating autoscaling, alerting, and pod eviction behavior.

## Build & Development Commands

```bash
# Build
cargo build --release

# Run locally (server on :8080)
cargo run

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Container build and run
docker compose up --build -d
docker compose logs -f
docker compose down

# Integration tests (requires running container)
./scripts/test-phase1.sh
./scripts/test-phase1.sh http://localhost:8080  # custom URL
```

## Architecture

Actor model with three layers:

1. **API Layer (Axum)** - Async HTTP handlers update shared state
2. **Orchestrator** - Background task watches state changes, manages stressor lifecycle
3. **Stressor Engines** - CPU/Memory use `std::thread` (bypass Tokio scheduler), Network uses async Tokio tasks

Only one stressor mode is active at a time. Mode switches trigger graceful shutdown before starting the new stressor.

### Source Structure

```
src/
├── main.rs          # Entry point, router setup
├── lib.rs           # Module exports
├── config.rs        # CpuConfig, MemoryConfig, NetworkConfig structs
├── state.rs         # SharedState (Arc<RwLock<AppState>>)
├── error.rs         # Error types
└── api/
    ├── mod.rs
    └── handlers.rs  # HTTP endpoint handlers
```

## Code Standards

### Required File Header
```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: YYYY-MM-DD
// License: MIT
```

### Rust Patterns
- No `unwrap()` in production - use `?` or `expect("reason")`
- CPU/Memory engines: use `std::thread` (not Tokio) to bypass async scheduler
- Network engine: use Tokio async tasks
- Stop signals: `Arc<AtomicBool>` for sync threads, `tokio::sync::watch` for async
- State sharing: `Arc<RwLock<T>>` for read-heavy access
- Error handling: `thiserror` for library errors, `anyhow` for application

### Anti-Patterns to Avoid
- `std::sync::Mutex` in async contexts (use `tokio::sync::Mutex`)
- `block_on()` inside async
- Unbounded channels
- Clone in hot loops

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/ready` | Readiness probe |
| GET | `/status` | Current mode and config |
| GET | `/metrics` | Prometheus metrics |
| POST | `/mode` | Set operation mode |
| POST | `/cpu` | Configure CPU stressor |
| POST | `/memory` | Configure memory stressor |
| POST | `/network` | Configure network stressor |
| POST | `/stop` | Stop all stressors |

## Key Algorithms

### CPU Control (PWM)
Uses 100ms windows with duty cycle = `target_milli / 1000`. Uses `std::hint::black_box()` to prevent compiler optimization of burn loops.

### Memory Pressure
Must dirty every 4KB page boundary for Linux to back virtual memory with physical RAM:
```rust
for i in (0..capacity).step_by(4096) {
    data[i] = 1;
}
```

## Kubernetes Deployment

Always set resource limits based on maximum intended stress:
```yaml
resources:
  requests:
    cpu: "100m"
    memory: "64Mi"
  limits:
    cpu: "4000m"
    memory: "2Gi"
```

Required labels: `app.kubernetes.io/name: k8s-stressor`

## Reference Documents

- `generated/technical-specification.md` - Full architecture details
- `generated/implementation-plan-version-1.md` - Implementation phases
- `.cascade/rules.md` - Project coding standards
