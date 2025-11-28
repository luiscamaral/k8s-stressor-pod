# k8s-stressor

> **Deterministic Resource Consumption for Kubernetes Reliability Testing**

[![Author](https://img.shields.io/badge/author-Luis%20Amaral-blue)](https://github.com/luiscamaral)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)

---

## Overview

**k8s-stressor** is a lightweight, purpose-built containerized agent designed to introduce controlled instability into Kubernetes clusters. Unlike standard load testing tools that target external endpoints, this application runs *inside* the cluster to simulate:

- **Noisy Neighbor** scenarios
- **Resource exhaustion** conditions
- **Pod lifecycle anomalies**

It serves as a precise internal stressor, enabling Site Reliability Engineers (SREs) and Platform Engineers to validate node behavior, autoscaling triggers, and observability pipelines—without requiring complex external frameworks.

---

## Core Capabilities

| Feature | Description |
|---------|-------------|
| **CPU Saturation** | Configurable multi-core consumption to test CPU throttling (Completely Fair Scheduler (CFS) quota) and Horizontal Pod Autoscaler (HPA) triggers |
| **Memory Pressure** | Synthetic RAM allocation to validate eviction policies, OOMKill behavior, and memory-based scaling rules |
| **Network Flood** | Internal bandwidth saturation to test NetworkPolicy throughput, Container Network Interface (CNI) limitations, and service mesh latency |
| **Keep-Alive Logic** | Foreground process that persists until a specific signal or resource limit is reached |

---

## Technical Stack

| Component | Technology |
|-----------|------------|
| **Language** | Rust (Edition 2021) |
| **Async Runtime** | Tokio (multi-threaded) |
| **Web Framework** | Axum 0.7 |
| **Metrics** | Prometheus |
| **Logging** | tracing |

**Architecture:** Simplified Actor Model with API Layer → Orchestrator → Stressor Engines

For detailed specifications, see [`generated/technical-specification.md`](generated/technical-specification.md).

---

## Use Cases

### HPA Validation
> *"If CPU hits 80%, does the ReplicaSet actually scale up?"*

Verify that your Horizontal Pod Autoscaler responds correctly to resource pressure thresholds.

### Alerting Drills
Confirm that Prometheus/Grafana pipelines correctly trigger alerts such as:
- `HighNodeCPUPressure`
- `HighNodeMemoryPressure`
- `PodEvictionThresholdReached`

### Taint & Toleration Testing
Ensure critical workloads can evict this stressor pod under pressure, validating your pod priority and preemption configurations.

### Quality of Service (QoS) Class Verification
Test how Kubernetes handles pods across different QoS classes:
- **Guaranteed** — Fixed resource requests/limits
- **Burstable** — Partial resource guarantees
- **BestEffort** — No resource guarantees (first to be evicted)

---

## Roadmap

### Phase 1: Advanced Simulation *(The "Realism" Update)*

| Feature | Description |
|---------|-------------|
| **Disk I/O Stress** | Heavy read/write operations on ephemeral storage to test IO wait impact on co-located pods |
| **Burstable Mode** | Sine wave or pulse patterns (e.g., 5min high / 2min low) to test autoscaler hysteresis |
| **Jitter/Randomness** | Random spikes in latency or consumption to simulate real production traffic unpredictability |

### Phase 2: Observability & Control *(The "SRE" Update)*

#### Prometheus Metrics Exporter

Expose a `/metrics` endpoint with gauges such as:

```promql
stressor_mode
stressor_cpu_target_millicores
stressor_cpu_actual_millicores
stressor_memory_allocated_bytes
stressor_is_active
```

*Overlay stressor intent against node reality on your dashboards.*

#### Remote Control API

Lightweight HTTP listener for dynamic control without redeployment:

```bash
# Set operation mode
curl -X POST http://stressor:8080/mode \
  -H "Content-Type: application/json" \
  -d '"cpu-stressor"'

# Configure CPU stress
curl -X POST http://stressor:8080/cpu \
  -H "Content-Type: application/json" \
  -d '{"mode":"linear","max_value":2000,"start_value":100,"growth_rate":50,"duration":120,"interval":10}'

# Get current status
curl http://stressor:8080/status

# Stop all stressors
curl -X POST http://stressor:8080/stop
```

### Phase 3: Chaos & Lifecycle *(The "Resilience" Update)*

#### Zombie Mode (Signal Trapping)

Configure the pod to trap `SIGTERM` signals and delay shutdown for a configurable duration.

**Purpose:** Test `terminationGracePeriodSeconds` configuration and verify monitoring catches stuck-terminating pods.

#### Liveness Probe Failure Simulation

Toggle to intentionally fail the `/healthz` endpoint.

**Purpose:** Verify Kubernetes restarts the pod as expected and that dependent services handle restart loops gracefully.

#### PodDisruptionBudget (PDB) Testing

Validate that evictions respect your PDB configurations during voluntary disruptions.

---

## Quick Start

```bash
# Build
cargo build --release

# Run locally
cargo run

# Test endpoints
curl http://localhost:8080/health
curl http://localhost:8080/status
```

For implementation details, see [`generated/implementation-plan-version-1.md`](generated/implementation-plan-version-1.md).

---

## API Reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Liveness probe |
| `GET` | `/status` | Current mode and configuration |
| `GET` | `/metrics` | Prometheus-format metrics |
| `POST` | `/mode` | Set operation mode |
| `POST` | `/cpu` | Configure CPU stressor |
| `POST` | `/memory` | Configure memory stressor |
| `POST` | `/network` | Configure network stressor |
| `POST` | `/stop` | Stop all stressors |

---

## Author

**Luis Amaral**

- GitHub: [@luiscamaral](https://github.com/luiscamaral)

---

## License

MIT License — Copyright (c) 2024 Luis Amaral