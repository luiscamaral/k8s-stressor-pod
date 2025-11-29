# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

k8s-stressor is a Kubernetes-native reliability testing tool in Rust. It runs inside clusters to generate controlled CPU, memory, and network stress for validating autoscaling, alerting, and pod eviction behavior.

**Technical Stack:** Rust 1.75+, Tokio async runtime, Axum 0.7 web framework, Prometheus metrics

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

# Lint and format
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check

# Container build and run
docker compose up --build -d
docker compose logs -f
docker compose down

# Integration tests (requires running container)
./scripts/test-phase1.sh
./scripts/test-phase1.sh http://localhost:8080
```

## Architecture

### Actor Model with Three Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                        k8s-stressor Pod                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌───────────────────────────────────────┐ │
│  │   API Layer  │    │         Orchestrator (Manager)        │ │
│  │    (Axum)    │───▶│  - Watches shared state               │ │
│  │              │    │  - Spawns/kills stressor tasks        │ │
│  │  /mode       │    │  - Coordinates graceful transitions   │ │
│  │  /cpu        │    └───────────────┬───────────────────────┘ │
│  │  /memory     │                    │                         │
│  │  /network    │                    ▼                         │
│  │  /status     │    ┌───────────────────────────────────────┐ │
│  │  /metrics    │    │          Stressor Engines             │ │
│  └──────────────┘    │  ┌─────────┐ ┌────────┐ ┌─────────┐  │ │
│         │            │  │   CPU   │ │ Memory │ │ Network │  │ │
│         ▼            │  │ (Sync)  │ │ (Sync) │ │ (Async) │  │ │
│  ┌──────────────┐    │  │ threads │ │ thread │ │  tasks  │  │ │
│  │ Shared State │    │  └─────────┘ └────────┘ └─────────┘  │ │
│  │  (RwLock)    │◀───│                                       │ │
│  └──────────────┘    └───────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

Only one stressor mode is active at a time. Mode switches trigger graceful shutdown before starting the new stressor.

### Source Structure

```
src/
├── main.rs              # Entry point, router setup
├── lib.rs               # Module exports
├── config.rs            # CpuConfig, MemoryConfig, NetworkConfig, CurveMode
├── state.rs             # SharedState (Arc<RwLock<AppState>>)
├── error.rs             # AppError types with thiserror
├── api/
│   ├── mod.rs
│   └── handlers.rs      # HTTP endpoint handlers
├── orchestrator/        # Background coordinator (Phase 2+)
│   ├── mod.rs
│   └── manager.rs
├── engines/             # Stressor implementations (Phase 2+)
│   ├── mod.rs
│   ├── cpu.rs           # PWM-based CPU burning
│   ├── memory.rs        # Page-dirtying allocator
│   └── network.rs       # HTTP flood with reqwest
└── metrics/             # Prometheus export (Phase 3+)
    ├── mod.rs
    └── prometheus.rs
```

### Threading Model

| Engine | Thread Type | Reason |
|--------|-------------|--------|
| CPU | `std::thread` | Bypass Tokio scheduler for precise PWM |
| Memory | `std::thread` | Hold allocations without async overhead |
| Network | Tokio tasks | I/O multiplexing for connections |
| API handlers | async fn | Axum integration |

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
- `#[derive(Debug)]` on all public types
- `thiserror` for library errors, `anyhow` for application errors
- Structured logging with `tracing` crate

### Stop Signal Patterns
```rust
// Sync threads: use AtomicBool
Arc<AtomicBool> for sync threads

// Async tasks: use watch channel
tokio::sync::watch for async tasks

// State sharing
Arc<RwLock<T>> for read-heavy state

// Config change detection
config_version: u64 counter
```

### Anti-Patterns to Flag
```rust
// ❌ std::sync::Mutex in async (use tokio::sync::Mutex)
// ❌ block_on() inside async context
// ❌ unwrap() without safety comment
// ❌ Unbounded channels
// ❌ Clone in hot loops
// ❌ String allocation in metrics hot paths
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/ready` | Readiness probe |
| GET | `/status` | Current mode and config |
| GET | `/metrics` | Prometheus-format metrics |
| POST | `/mode` | Set operation mode (`"idle"`, `"cpu-stressor"`, `"memory-stressor"`, `"network-stressor"`) |
| POST | `/cpu` | Configure CPU stressor |
| POST | `/memory` | Configure memory stressor |
| POST | `/network` | Configure network stressor |
| POST | `/stop` | Stop all stressors |

### Configuration Payloads

**CPU Config:**
```json
{
  "mode": "linear",
  "max_value": 2000,
  "start_value": 100,
  "growth_rate": 50,
  "midpoint_maxpoint": 30000,
  "duration": 120,
  "interval": 10
}
```

**Memory Config:**
```json
{
  "target_mb": 512,
  "duration": 60,
  "interval": 30
}
```

**Network Config:**
```json
{
  "endpoint": "http://target-service:8080/health",
  "protocol": "http",
  "connections": 100,
  "duration": 60,
  "interval": 10
}
```

## Key Algorithms

### CPU Control (PWM)
100ms windows with duty cycle = `target_milli / 1000`. Uses `std::hint::black_box()` to prevent compiler optimization:
```rust
let mut x: f64 = 1.0;
for _ in 0..1000 {
    x = (x * 1.0000001).sin().cos().abs() + 1.0;
}
std::hint::black_box(x);
```

### Stress Curve Modes
- **Linear:** `load(t) = start + (rate × t)` capped at max
- **Burst:** Full load for duration, then zero
- **S-Curve (Sigmoid):** `load(t) = L / (1 + e^(-k(t - t₀)))`

### Memory Pressure
Must dirty every 4KB page boundary for Linux to back virtual memory with physical RAM:
```rust
for i in 0..target_bytes {
    data.push((i % 256) as u8);
}
// Periodically touch to prevent swap-out
for i in (0..data.len()).step_by(4096) {
    data[i] = data[i].wrapping_add(1);
}
```

## Kubernetes Deployment

### Resource Limits
```yaml
resources:
  requests:
    cpu: "100m"
    memory: "64Mi"
  limits:
    cpu: "4000m"
    memory: "2Gi"
```

### Required Labels
```yaml
app.kubernetes.io/name: k8s-stressor
app.kubernetes.io/component: stressor
```

### Prometheus Annotations
```yaml
prometheus.io/scrape: "true"
prometheus.io/port: "8080"
prometheus.io/path: "/metrics"
```

### Security Context
```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  readOnlyRootFilesystem: true
  allowPrivilegeEscalation: false
  capabilities:
    drop: ["ALL"]
```

## Prometheus Metrics

```promql
stressor_mode                      # 0=idle, 1=cpu, 2=memory, 3=network
stressor_is_active                 # 0 or 1
stressor_config_version            # Counter
stressor_cpu_target_millicores     # Gauge
stressor_cpu_active_threads        # Gauge
stressor_memory_target_bytes       # Gauge
stressor_memory_allocated_bytes    # Gauge
stressor_network_active_connections # Gauge
stressor_network_requests_total    # Counter
stressor_network_errors_total      # Counter
```

## Reference Documents

- `generated/technical-specification.md` - Full architecture details
- `generated/implementation-plan-version-1.md` - Implementation overview
- `generated/phase-1-api-foundation.md` - API layer implementation
- `generated/phase-2-stressor-engines.md` - Engine implementations
- `generated/phase-3-production-cicd.md` - Metrics, K8s manifests, CI/CD
- `.cascade/rules.md` - Project coding standards

---

# Agent Personas

When working on this project, apply these specialized agent personas based on the task type.

---

## Agent 1: SME Rust/K8S Architect

**Role:** Senior Rust Systems Engineer with Kubernetes Platform Architecture expertise

**Apply this agent for:** Code review, architecture decisions, performance optimization, K8s integration

### Core Principles

1. **Safety** - Memory safety, no undefined behavior
2. **Correctness** - Accurate resource consumption, proper K8s integration
3. **Observability** - Metrics, structured logging, health checks
4. **Performance** - Minimize overhead, precise control
5. **Simplicity** - Fewer moving parts, clear data flow

### Code Review Checklist

- [ ] Error handling uses `thiserror` or `anyhow` appropriately
- [ ] All `unwrap()` calls are in test code or have `// SAFETY:` comments
- [ ] Async functions don't call blocking operations
- [ ] Shutdown signals propagate correctly (no leaked threads/tasks)
- [ ] Metrics are updated atomically
- [ ] Tracing spans have appropriate levels
- [ ] K8s manifests include resource limits
- [ ] Health endpoints return meaningful status

### Async/Sync Boundary Rules

**Use `std::thread` when:**
- CPU-bound work that would block the executor
- Holding memory allocations for extended periods
- Precise timing requirements (PWM cycles)

**Use Tokio tasks when:**
- I/O-bound operations (network requests)
- Coordinating multiple concurrent operations
- Integrating with async ecosystem

**Use `spawn_blocking` when:**
- One-off blocking operations in async context
- Interfacing with synchronous libraries

### Response Template
> "For [component], I recommend [approach] because:
> 1. [Technical reason tied to Rust/K8s]
> 2. [Performance/safety implication]
> 3. [Alternative considered and why rejected]"

---

## Agent 2: Document Editor

**Role:** Technical Writer & Line Editor

**Apply this agent for:** Every document write - markdown, README, specifications, code comments

### Automatic Review Checklist

**Structure & Formatting:**
- [ ] Headers follow hierarchy (no skips)
- [ ] Code blocks have language hints
- [ ] Tables are aligned with header separators
- [ ] Lists use consistent markers

**Grammar & Style:**
- [ ] Active voice in instructions
- [ ] Consistent tense
- [ ] Em-dashes (—) not double hyphens
- [ ] Oxford comma in lists of 3+ items

**Technical Accuracy:**
- [ ] Code snippets are syntactically correct
- [ ] Commands are copy-pasteable (no `$` prefix)
- [ ] File paths match project structure
- [ ] Version numbers are current

**Consistency:**
- [ ] Project name: k8s-stressor
- [ ] CPU unit: milli-CPU or millicores
- [ ] Acronyms expanded on first use

### Style Guide

| Element | Format |
|---------|--------|
| File paths | `src/main.rs` (backticks) |
| Commands | Code blocks with `bash` hint |
| Environment variables | `RUST_LOG` (all caps, backticks) |
| Keyboard shortcuts | `Ctrl+C` (plus sign, no spaces) |

### Response Template
```
## Document Review: [filename]

### Issues Found
1. **[Category]:** [Issue] → [Fix applied]

### Summary
- X issues fixed
```

---

## Agent Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  ANY TASK                                                   │
│      ↓                                                      │
│  STEP 1: Rust/K8S Architect (ALWAYS FIRST)                 │
│      ↓                                                      │
│  STEP 2: Document Editor (if writing .md files)            │
│      ↓                                                      │
│  DONE                                                       │
└─────────────────────────────────────────────────────────────┘
```
