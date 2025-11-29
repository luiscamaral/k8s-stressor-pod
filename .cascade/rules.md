# k8s-stressor Project Rules

---

## ⚡ AGENT WORKFLOW (Apply in Order)

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

### Step 1: Rust/K8S Architect — PRIMARY
> **Trigger:** ALL tasks (code, config, architecture, reviews)  
> **Reference:** `agents/rust-k8s-architect.md`  
> **Applies:** Rust patterns, async/sync boundaries, K8s standards, error handling

**Always enforce:**
- No `unwrap()` in production
- Correct threading model (sync for CPU/Memory, async for Network)
- K8s resource limits and labels
- Prometheus metrics naming

### Step 2: Document Editor — SECONDARY
> **Trigger:** After writing `.md` files (runs AFTER Architect review)  
> **Reference:** `agents/document-editor.md`  
> **Action:** Review formatting and fix issues

**Checklist:**
- [ ] Headers follow hierarchy (no skips)
- [ ] Code blocks have language hints
- [ ] Active voice in instructions
- [ ] Commands are copy-pasteable (no `$` prefix)
- [ ] Consistent terminology (k8s-stressor, milli-CPU)
- [ ] Acronyms expanded on first use

---

## Source Code File Headers

**REQUIRED** on every source code file:

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: YYYY-MM-DD
// License: MIT
```

**Apply to:** `.rs`, `Dockerfile`, K8s manifests, shell scripts

---

## Technical Stack

- **Language:** Rust (Edition 2021)
- **Runtime:** Tokio (multi-threaded async)
- **Framework:** Axum 0.7
- **Target:** Kubernetes pod deployment

## Architecture Pattern

Actor Model with three layers:
1. **API (Axum):** Async HTTP handlers → shared state
2. **Orchestrator:** Background task watching state changes
3. **Engines:** CPU (sync threads), Memory (sync), Network (async)

---

## Rust Code Standards

### Required
- No `unwrap()` in production code — use `?` or `expect("reason")`
- No blocking in async contexts — use `std::thread` or `spawn_blocking`
- `#[derive(Debug)]` on all public types
- `thiserror` for library errors, `anyhow` for application errors
- Structured logging with `tracing` crate

### Threading Model
| Component | Use | Reason |
|-----------|-----|--------|
| CPU burn | `std::thread` | Bypass Tokio scheduler |
| Memory hold | `std::thread` | Long-lived allocations |
| Network flood | Tokio tasks | I/O multiplexing |
| API handlers | async fn | Axum integration |

### Patterns to Use
```rust
// Stop signals
Arc<AtomicBool> for sync threads
tokio::sync::watch for async tasks

// State sharing  
Arc<RwLock<T>> for read-heavy state

// Config version tracking
config_version: u64 counter for change detection
```

### Anti-Patterns to Flag
```rust
// ❌ std::sync::Mutex in async (use tokio::sync::Mutex)
// ❌ block_on() inside async context
// ❌ unwrap() without safety comment
// ❌ Unbounded channels
// ❌ Clone in hot loops
```

---

## Kubernetes Standards

### Resource Limits (Always Set)
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

---

## Project Structure

```
src/
├── main.rs           # Entry point
├── config.rs         # CpuConfig, MemoryConfig, NetworkConfig
├── state.rs          # SharedState (Arc<RwLock<AppState>>)
├── api/handlers.rs   # HTTP handlers
├── orchestrator/     # Background coordinator
├── engines/          # cpu.rs, memory.rs, network.rs
└── metrics/          # Prometheus export
```

## Specification Documents
- `generated/technical-specification.md` — Architecture
- `generated/implementation-plan-version-1.md` — Phases
- `AGENT_RUST_K8S_ARCHITECT.md` — Full agent definition

---

## Key Algorithms

### CPU Control (PWM)
- 100ms windows
- Duty cycle = `target_milli / 1000`
- Use `std::hint::black_box()` to prevent optimization

### Memory Pressure
- Must dirty every 4KB page
- Write actual bytes, not just capacity

### S-Curve Formula
```
load(t) = L / (1 + e^(-k(t - t₀)))
```

---

## Commands

```bash
cargo build --release     # Production build
cargo run                 # Dev server :8080
cargo test               # Run tests
RUST_LOG=debug cargo run # Verbose logging
```

## API Endpoints
- `GET /health` — Liveness
- `GET /status` — Current state
- `GET /metrics` — Prometheus
- `POST /mode` — Set mode
- `POST /cpu|memory|network` — Configure
- `POST /stop` — Stop all
