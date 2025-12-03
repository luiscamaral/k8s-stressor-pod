# Contributing to k8s-stressor

Thank you for your interest in contributing to k8s-stressor! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

---

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you are expected to uphold this code.

---

## Getting Started

### Prerequisites

- **Rust 1.75+** — Install via [rustup](https://rustup.rs/) or [mise](https://mise.jdx.dev/)
- **Docker** — For building and testing container images
- **kubectl** — For Kubernetes testing (optional)

### Setup

```bash
# Clone the repository
git clone https://github.com/luiscamaral/k8s-stressor-pod.git
cd k8s-stressor-pod

# Install Rust toolchain (if using mise)
mise install

# Build the project
cargo build

# Run tests
cargo test
```

---

## Development Workflow

### Branch Naming

| Type | Format | Example |
|------|--------|---------|
| Feature | `feature/<description>` | `feature/disk-io-stressor` |
| Bug fix | `fix/<description>` | `fix/memory-leak` |
| Documentation | `docs/<description>` | `docs/api-guide` |
| Chore | `chore/<description>` | `chore/update-deps` |

### Making Changes

1. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make your changes** following the [coding standards](#coding-standards)

3. **Test your changes**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```

4. **Commit with conventional commits**:
   ```bash
   git commit -m "feat: add disk I/O stressor"
   ```

5. **Push and create a Pull Request**:
   ```bash
   git push origin feature/my-feature
   ```

### Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation only
- `style` — Code style (formatting, etc.)
- `refactor` — Code change that neither fixes a bug nor adds a feature
- `perf` — Performance improvement
- `test` — Adding or updating tests
- `chore` — Maintenance tasks

**Examples:**
```
feat(cpu): add burst mode support
fix(memory): prevent allocation overflow
docs: update API reference
chore: bump tokio to 1.35
```

---

## Pull Request Process

### Before Submitting

- [ ] Code follows the [coding standards](#coding-standards)
- [ ] All tests pass (`cargo test`)
- [ ] Linting passes (`cargo clippy`)
- [ ] Formatting is correct (`cargo fmt --check`)
- [ ] Documentation is updated (if applicable)
- [ ] Commit messages follow conventional commits

### PR Description

Include:
- **What** — Brief description of changes
- **Why** — Motivation or issue reference
- **How** — Implementation approach (if complex)
- **Testing** — How you tested the changes

### Review Process

1. CI checks must pass
2. At least one maintainer approval required
3. All conversations must be resolved
4. Branch must be up-to-date with `main`

### After Merge

- PR images are available at `ghcr.io/luiscamaral/k8s-stressor-pod:pr-<number>`
- Delete your branch after merge

---

## Coding Standards

### Rust Guidelines

- **Edition:** 2021
- **MSRV:** 1.75
- **Formatting:** Use `rustfmt` (default settings)
- **Linting:** Zero `clippy` warnings

### Error Handling

```rust
// ✅ Good: Use Result with thiserror
#[derive(Debug, thiserror::Error)]
pub enum StressorError {
    #[error("configuration invalid: {0}")]
    Config(String),
}

// ❌ Bad: Using unwrap in production code
let value = config.get("key").unwrap();
```

### File Headers

All source files must include a header:

```rust
// Copyright (c) 2024 Luis Amaral
// Author: Luis Amaral
// Created: YYYY-MM-DD
// License: MIT
```

### Async/Sync Boundaries

| Work Type | Thread Model |
|-----------|-------------|
| I/O bound | Tokio async tasks |
| CPU bound | `std::thread` |
| Memory allocation | `std::thread` |

---

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_cpu_linear_ramp

# Run with output
cargo test -- --nocapture
```

### Integration Tests

```bash
# Start the application
docker compose up -d

# Run integration tests
./scripts/test-phase1.sh http://localhost:8080
./scripts/test-phase2.sh http://localhost:8080
./scripts/test-phase3.sh http://localhost:8080

# Stop
docker compose down
```

### Code Coverage

```bash
# Install coverage tool
cargo install cargo-llvm-cov

# Generate coverage report
./scripts/coverage.sh html
```

---

## Documentation

### Where to Document

| Content | Location |
|---------|----------|
| API changes | `README.md`, OpenAPI annotations |
| Architecture | `generated/` specs |
| Build/Release | `docs/build-and-release.md` |
| Code | Inline rustdoc comments |

### Rustdoc Style

```rust
/// Calculates target CPU load for cyclic behavior.
///
/// # Arguments
///
/// * `config` - CPU stressor configuration
/// * `elapsed_ms` - Time elapsed in current cycle
///
/// # Returns
///
/// Target load in millicores (0-max_value)
///
/// # Example
///
/// ```
/// let load = calculate_load_cyclic(&config, 5000, 30000, 10000);
/// ```
pub fn calculate_load_cyclic(...) -> f64 {
```

---

## Questions?

- Open a [GitHub Discussion](https://github.com/luiscamaral/k8s-stressor-pod/discussions)
- Check existing [Issues](https://github.com/luiscamaral/k8s-stressor-pod/issues)

Thank you for contributing! 🎉
