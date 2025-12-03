# Build and Release Process

> **Author:** Luis Amaral  
> **Created:** 2024-12-02

This document describes the build, CI/CD, and release workflows for k8s-stressor.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [CI/CD Pipeline](#cicd-pipeline)
- [Docker Images](#docker-images)
- [Release Process](#release-process)
- [Local Development](#local-development)
- [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         GitHub Repository                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Push/PR ──────► CI Workflow ──────► GHCR (temporary images)        │
│                  (ci.yml)            pr-<number> or <sha>           │
│                                                                      │
│  Tag v* ───────► Release Workflow ──► GHCR (release images)         │
│                  (release.yml)        <version>, latest             │
│                                       + GitHub Release               │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Principles

- **All compilation happens inside Docker** — No local Rust toolchain required
- **Multi-arch images** — Both `linux/amd64` and `linux/arm64` supported
- **Immutable releases** — Version tags trigger official releases
- **Temporary CI images** — PR and commit images for testing, not production

---

## CI/CD Pipeline

### Workflow: `.github/workflows/ci.yml`

Triggered on:
- Push to `main` or `develop` branches
- Pull requests to `main` or `develop`

#### Jobs

| Job | Description |
|-----|-------------|
| **lint** | Format check (`cargo fmt`) and linting (`cargo clippy`) |
| **test** | Unit tests in debug and release mode |
| **build** | Multi-arch Docker image build and push to GHCR |
| **integration** | Run integration tests against built image |
| **security** | Trivy vulnerability scanning |
| **k8s-validate** | Validate Kubernetes manifests with kubeconform |

#### Image Tags

| Event | Tag Format | Example |
|-------|------------|---------|
| Pull Request | `pr-<number>` | `pr-42` |
| Push to branch | `<short-sha>` | `abc1234` |

### Pulling CI Images

```bash
# Pull a PR build for testing
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:pr-42

# Pull a specific commit
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:abc1234
```

---

## Docker Images

### Registry

All images are published to **GitHub Container Registry (GHCR)**:

```
ghcr.io/luiscamaral/k8s-stressor-pod
```

### Multi-Architecture Support

Images are built for:
- `linux/amd64` (x86_64)
- `linux/arm64` (Apple Silicon, AWS Graviton, etc.)

Docker automatically selects the correct architecture when pulling.

### Build Process

The `Dockerfile` uses a **two-stage build**:

1. **Builder stage** (`rust:1.75-slim-bookworm`)
   - Installs build dependencies
   - Caches Cargo dependencies
   - Compiles release binary

2. **Runtime stage** (`debian:bookworm-slim`)
   - Minimal base image (~80MB)
   - Non-root user (UID 1000)
   - Only runtime dependencies (ca-certificates, curl)

---

## Release Process

### Workflow: `.github/workflows/release.yml`

Triggered by pushing a version tag (`v*`).

#### Prerequisites

1. Update version in `Cargo.toml`
2. Commit changes
3. Create and push tag

#### Steps

```bash
# 1. Update Cargo.toml version
vim Cargo.toml  # Change version = "0.4.0"

# 2. Commit the version bump
git add Cargo.toml
git commit -m "chore: bump version to 0.4.0"

# 3. Create annotated tag
git tag -a v0.4.0 -m "Release v0.4.0"

# 4. Push commit and tag
git push origin main
git push origin v0.4.0
```

#### What Happens

1. **Validate** — Verifies `Cargo.toml` version matches tag
2. **Test** — Runs full test suite
3. **Build** — Creates multi-arch Docker image
4. **Push** — Pushes to GHCR with version tags
5. **Release** — Creates GitHub Release with changelog

#### Release Image Tags

| Tag | Description |
|-----|-------------|
| `0.4.0` | Full semver |
| `0.4` | Minor version (updated on patch releases) |
| `0` | Major version (updated on minor/patch releases) |
| `latest` | Always points to newest release |

### Pulling Release Images

```bash
# Specific version (recommended for production)
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:0.4.0

# Latest release
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:latest

# Minor version tracking (auto-updates for patches)
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:0.4
```

### Pre-releases

Tags containing `-` are marked as pre-releases:

```bash
git tag -a v0.5.0-rc.1 -m "Release candidate 1"
git push origin v0.5.0-rc.1
```

---

## Local Development

### Building Locally

The `scripts/build-and-push.sh` script builds images using the multi-stage Dockerfile.

```bash
# Build for native architecture (recommended)
./scripts/build-and-push.sh --skip-push

# Build with custom tag
./scripts/build-and-push.sh --tag dev --skip-push
```

### Multi-Architecture Builds

Local multi-arch builds use QEMU emulation, which can be slow or crash for Rust compilation.

**Recommended:** Use CI for multi-arch builds.

```bash
# If you must build multi-arch locally (may fail on ARM Macs)
./scripts/build-and-push.sh --multi-arch --skip-push
```

### Running Locally

```bash
# Using Docker Compose
docker compose up --build

# Or run the built image directly
docker run -p 8080:8080 ghcr.io/luiscamaral/k8s-stressor-pod:latest
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests (requires running container)
./scripts/test-phase1.sh http://localhost:8080
./scripts/test-phase2.sh http://localhost:8080
./scripts/test-phase3.sh http://localhost:8080
```

---

## Troubleshooting

### QEMU Emulation Failures

**Symptom:** Build crashes with `SIGSEGV` or `signal 11` when building for non-native architecture.

**Cause:** QEMU emulation of Rust compilation is unreliable.

**Solution:** 
- Build for native architecture only: `./scripts/build-and-push.sh --skip-push`
- Use CI for multi-arch builds

### Image Not Found After PR

**Symptom:** Cannot pull `pr-<number>` image.

**Cause:** PR images require GHCR authentication for private repos.

**Solution:**
```bash
# Authenticate with GHCR
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

# Then pull
docker pull ghcr.io/luiscamaral/k8s-stressor-pod:pr-42
```

### Version Mismatch Error in Release

**Symptom:** Release workflow fails with "Version mismatch" error.

**Cause:** `Cargo.toml` version doesn't match git tag.

**Solution:** Ensure the version in `Cargo.toml` matches the tag (without `v` prefix):
- Tag: `v0.4.0`
- Cargo.toml: `version = "0.4.0"`

### Build Cache Issues

**Symptom:** CI builds are slow or using stale dependencies.

**Solution:** GitHub Actions cache can be cleared from the Actions tab → Caches.

---

## See Also

- [README.md](../README.md) — Project overview and quick start
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Contribution guidelines
- [generated/](../generated/) — Technical specifications
