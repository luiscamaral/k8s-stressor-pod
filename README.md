# k8s-stressor

> **Deterministic Resource Consumption for Kubernetes Reliability Testing**

[![Repository](https://img.shields.io/badge/repo-k8s--stressor--pod-purple)](https://github.com/luiscamaral/k8s-stressor-pod)
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
curl -X PUT http://stressor:8080/mode \
  -H "Content-Type: application/json" \
  -d '"cpu-stressor"'

# Configure CPU stress
curl -X PUT http://stressor:8080/config/cpu \
  -H "Content-Type: application/json" \
  -d '{"mode":"linear","max_value":2000,"start_value":100,"growth_rate":50,"midpoint_ms":60000,"interval":10}'

# Get current status
curl http://stressor:8080/status

# Stop all stressors
curl -X PUT http://stressor:8080/stop
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

### Prerequisites

```bash
# Install Rust 1.75 via mise (recommended)
mise install

# Or use rustup
rustup install 1.75
```

### Local Development

```bash
# Build
cargo build --release

# Run locally
cargo run

# Test endpoints
curl http://localhost:8080/health
curl http://localhost:8080/status
```

### Container (Docker/Nerdctl)

```bash
# Build and start with compose
docker compose up --build -d
# Or with nerdctl
nerdctl compose up --build -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

### Testing

```bash
# Run unit tests
cargo test

# Run integration tests (start container first)
./scripts/test-phase1.sh   # API foundation tests
./scripts/test-phase2.sh   # Stressor engine tests
./scripts/test-phase3.sh   # Prometheus metrics and production tests

# Specify custom URL
./scripts/test-phase3.sh http://localhost:8080
```

### Code Coverage

```bash
# Install coverage tool (one-time)
cargo install cargo-llvm-cov@0.6.15 --locked

# Run coverage
./scripts/coverage.sh summary   # Terminal summary
./scripts/coverage.sh html      # HTML report (opens browser)
./scripts/coverage.sh lcov      # LCOV format for CI
```

For implementation details, see [`generated/implementation-plan-version-1.md`](generated/implementation-plan-version-1.md).

---

## API Reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Liveness probe |
| `GET` | `/ready` | Readiness probe |
| `GET` | `/status` | Runtime status (mode, config_version, is_active) |
| `GET` | `/metrics` | Prometheus-format metrics |
| `GET` | `/mode` | Get current operation mode |
| `PUT` | `/mode` | Set operation mode |
| `GET` | `/config/cpu` | Get CPU configuration |
| `PUT` | `/config/cpu` | Configure CPU stressor |
| `GET` | `/config/memory` | Get memory configuration |
| `PUT` | `/config/memory` | Configure memory stressor |
| `GET` | `/config/network` | Get network configuration |
| `PUT` | `/config/network` | Configure network stressor |
| `GET` | `/config/disk` | Get disk I/O configuration |
| `PUT` | `/config/disk` | Configure disk I/O stressor |
| `GET` | `/config/chaos` | Get chaos/lifecycle configuration |
| `PUT` | `/config/chaos` | Configure chaos/lifecycle simulation |
| `PUT` | `/stop` | Stop all stressors |

Interactive API documentation available at `/swagger-ui/`.

### Disk I/O Stressor Examples

**Stress IOPS** (random access, database-like workloads):

```bash
# High IOPS: 4KB blocks + random pattern = ~12,800 IOPS at 50 MB/s
curl -X PUT http://localhost:8080/config/disk \
  -H "Content-Type: application/json" \
  -d '{
    "target_mbps": 50,
    "pattern": "random",
    "block_size_kb": 4,
    "read_ratio": 0.7
  }'

curl -X PUT http://localhost:8080/mode -H "Content-Type: application/json" -d '"disk-stressor"'
```

**Stress Throughput** (sequential I/O, large file operations):

```bash
# High throughput: 256KB blocks + sequential pattern = ~400 IOPS at 100 MB/s
curl -X PUT http://localhost:8080/config/disk \
  -H "Content-Type: application/json" \
  -d '{
    "target_mbps": 100,
    "pattern": "sequential",
    "block_size_kb": 256,
    "read_ratio": 0.5
  }'

curl -X PUT http://localhost:8080/mode -H "Content-Type: application/json" -d '"disk-stressor"'
```

**Multi-volume testing:**

```bash
# Test multiple storage classes simultaneously
curl -X PUT http://localhost:8080/config/disk \
  -H "Content-Type: application/json" \
  -d '{
    "work_dir": "/tmp/k8s-stressor",
    "additional_paths": ["/mnt/fast-ssd", "/mnt/standard-hdd"],
    "target_mbps": 50,
    "pattern": "random",
    "block_size_kb": 4
  }'
```

**Key:** `Throughput (MB/s) = IOPS × Block Size (KB) / 1024`

---

## Kubernetes Deployment

Kubernetes manifests are provided in the `k8s/` directory:

```bash
# Deploy to cluster
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -l app.kubernetes.io/name=k8s-stressor

# View logs
kubectl logs -l app.kubernetes.io/name=k8s-stressor -f

# Access the API (port-forward)
kubectl port-forward svc/k8s-stressor 8080:8080
```

The deployment includes:
- **Prometheus annotations** for automatic metrics scraping
- **Security context** with non-root user, read-only filesystem, and dropped capabilities
- **Resource limits** (100m-4000m CPU, 128Mi-2Gi memory)
- **Liveness and readiness probes**

---

## CI/CD

The project includes a GitHub Actions workflow (`.github/workflows/ci.yml`) that runs on push to `main`/`develop` branches and pull requests.

**Pipeline stages:**
1. **Lint** - `cargo fmt` and `cargo clippy`
2. **Test** - Unit tests in debug and release mode
3. **Build** - Docker image build with BuildKit caching
4. **Integration** - Run all integration test scripts
5. **Security** - Trivy vulnerability scanning
6. **K8s Validation** - Validate Kubernetes manifests

Container images are published to `ghcr.io/luiscamaral/k8s-stressor-pod`.

For detailed build and release documentation, see [`docs/build-and-release.md`](docs/build-and-release.md).

---

## Contributing

We welcome contributions! Please see [`CONTRIBUTING.md`](CONTRIBUTING.md) for guidelines.

---

## Author

**Luis Amaral**

- GitHub: [@luiscamaral](https://github.com/luiscamaral)

---

## License

MIT License — Copyright (c) 2024 Luis Amaral