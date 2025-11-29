# k8s-stressor Technical Specification

> **Version:** 1.0  
> **Status:** Draft  
> **Language:** Rust  
> **Runtime:** Tokio + Axum

---

## Executive Summary

**k8s-stressor** is a Kubernetes-native reliability testing tool built in Rust. It provides deterministic, controllable resource consumption for validating cluster behavior under stress conditions.

For detailed implementation phases and coding guidelines, see:  
→ [`implementation-plan-version-1.md`](./implementation-plan-version-1.md)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        k8s-stressor Pod                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
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
│  └──────────────┘    │                                       │ │
│         │            │  ┌─────────┐ ┌────────┐ ┌─────────┐  │ │
│         │            │  │   CPU   │ │ Memory │ │ Network │  │ │
│         ▼            │  │ (Sync)  │ │ (Sync) │ │ (Async) │  │ │
│  ┌──────────────┐    │  │ threads │ │ thread │ │  tasks  │  │ │
│  │ Shared State │    │  └─────────┘ └────────┘ └─────────┘  │ │
│  │  (RwLock)    │◀───│                                       │ │
│  └──────────────┘    └───────────────────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Design Pattern: Simplified Actor Model

| Component | Responsibility | Threading Model |
|-----------|---------------|-----------------|
| **API Layer** | HTTP request handling, state updates | Async (Tokio) |
| **Orchestrator** | State change detection, task lifecycle | Async (Tokio) |
| **CPU Engine** | Raw CPU cycle burning via PWM | Sync (`std::thread`) |
| **Memory Engine** | RAM allocation with page dirtying | Sync (managed thread) |
| **Network Engine** | Connection flooding | Async (Tokio tasks) |

---

## Operation Modes

```rust
pub enum OperationMode {
    CpuStressor,      // CPU saturation active
    MemoryStressor,   // Memory pressure active
    NetworkStressor,  // Network flood active
    Idle,             // No active stress
}
```

Only **one mode** is active at a time. Switching modes triggers graceful shutdown of the current stressor before starting the new one.

---

## Stress Curve Models

All stressors support configurable load curves:

### Linear
```
Load
  │      ╱────────── max
  │    ╱
  │  ╱
  │╱
  └──────────────────▶ Time
     start
```
**Formula:** `load(t) = start + (rate × t)`

### Burst
```
Load
  │ ████████████
  │ █          █
  │ █          █
  │ █          █
  └─█──────────█────▶ Time
    on        off
```
**Formula:** Full load for duration, then zero

### S-Curve (Sigmoid)
```
Load
  │           ╭────── max
  │         ╱
  │       ╱
  │     ╱
  │ ──╯
  └──────────────────▶ Time
         midpoint
```
**Formula:** `load(t) = L / (1 + e^(-k(t - t₀)))`

---

## API Contract

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/status` | Current mode and active configuration |
| `GET` | `/metrics` | Prometheus-format metrics |
| `POST` | `/mode` | Set operation mode |
| `POST` | `/cpu` | Configure CPU stressor |
| `POST` | `/memory` | Configure memory stressor |
| `POST` | `/network` | Configure network stressor |
| `POST` | `/stop` | Stop all stressors (set to Idle) |

### Configuration Payloads

#### CPU Configuration
```json
{
  "mode": "s-curve",
  "max_value": 2000,
  "start_value": 100,
  "growth_rate": 50,
  "midpoint_maxpoint": 30000,
  "duration": 120,
  "interval": 10
}
```

| Field | Unit | Description |
|-------|------|-------------|
| `max_value` | milli-CPU | Maximum load (2000 = 2 cores) |
| `start_value` | milli-CPU | Initial load |
| `growth_rate` | per-ms | Rate of increase |
| `midpoint_maxpoint` | ms | Sigmoid midpoint or burst duration |
| `duration` | seconds | Total stress duration |
| `interval` | seconds | Rest period between cycles |

#### Memory Configuration
```json
{
  "target_mb": 512,
  "duration": 60,
  "interval": 30
}
```

#### Network Configuration
```json
{
  "endpoint": "http://target-service:8080/health",
  "protocol": "http",
  "connections": 100,
  "duration": 60,
  "interval": 10
}
```

---

## Metrics Export

Prometheus-compatible metrics at `/metrics`:

```promql
# HELP stressor_mode Current operation mode (0=idle, 1=cpu, 2=memory, 3=network)
stressor_mode 1

# HELP stressor_cpu_target_millicores Target CPU load in millicores
stressor_cpu_target_millicores 1500

# HELP stressor_cpu_actual_millicores Actual CPU load being applied
stressor_cpu_actual_millicores 1487

# HELP stressor_memory_allocated_bytes Current memory allocation
stressor_memory_allocated_bytes 536870912

# HELP stressor_network_active_connections Active network connections
stressor_network_active_connections 95

# HELP stressor_is_active Whether a stressor is currently running
stressor_is_active 1
```

---

## Key Technical Considerations

### CPU Stressor: PWM-Based Load Control

To achieve precise milli-CPU control, we use **Pulse Width Modulation**:

- **Active phase:** Tight calculation loop (burns cycles)
- **Passive phase:** `thread::sleep`
- **Window:** 100ms periods

**Example:** 500m (0.5 cores) = 50ms active + 50ms sleep per window

### Memory Stressor: Page Dirtying

Linux uses lazy allocation—virtual memory isn't backed by physical RAM until written. We must **dirty pages** for Kubernetes metrics to reflect actual usage:

```rust
// Write to every 4KB page boundary
for i in (0..capacity).step_by(4096) {
    data[i] = 1;
}
```

### Network Stressor: Connection Pooling

Use `reqwest` with configurable connection pools to maintain sustained pressure without exhausting file descriptors.

---

## Deployment Model

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: k8s-stressor
spec:
  replicas: 1
  template:
    spec:
      containers:
      - name: stressor
        image: k8s-stressor:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: "100m"
            memory: "64Mi"
          limits:
            cpu: "4000m"      # Allow up to 4 cores
            memory: "2Gi"     # Allow up to 2GB
```

> **Note:** Set resource limits based on maximum intended stress levels.

---

## References

- [Implementation Plan v1](./implementation-plan-version-1.md) — Detailed phases and code structure
- [README](../README.md) — Project overview and use cases
