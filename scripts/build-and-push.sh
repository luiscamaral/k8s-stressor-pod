#!/usr/bin/env bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-12-02
#
# Two-phase Docker build script for k8s-stressor
# Phase 1: Cross-compile Rust binary for x86_64
# Phase 2: Package into Docker image and push to ECR
#
# Usage:
#   ./scripts/build-and-push.sh                    # Build and push with default settings
#   ./scripts/build-and-push.sh --skip-push        # Build only, don't push
#   ./scripts/build-and-push.sh --tag v1.0.0       # Use custom tag

set -euo pipefail

# Configuration
ECR_REGISTRY="${ECR_REGISTRY:-506741563541.dkr.ecr.us-east-1.amazonaws.com}"
ECR_REPO="${ECR_REPO:-k8s-stressor}"
AWS_REGION="${AWS_REGION:-us-east-1}"
TARGET="${TARGET:-x86_64-unknown-linux-musl}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

# Parse arguments
SKIP_PUSH=false
CUSTOM_TAG=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-push)
            SKIP_PUSH=true
            shift
            ;;
        --tag)
            CUSTOM_TAG="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

TAG="${CUSTOM_TAG:-$VERSION}"
IMAGE="${ECR_REGISTRY}/${ECR_REPO}"

echo "=== k8s-stressor Build & Push ==="
echo "Version: ${VERSION}"
echo "Tag: ${TAG}"
echo "Target: ${TARGET}"
echo "Image: ${IMAGE}"
echo ""

# Phase 1: Build binary
echo "=== Phase 1: Building Rust binary for ${TARGET} ==="

# Check if cross is available (preferred for cross-compilation)
if command -v cross &> /dev/null; then
    echo "Using 'cross' for cross-compilation..."
    cross build --release --target "${TARGET}"
else
    # Fallback: check if target is installed
    if rustup target list --installed | grep -q "${TARGET}"; then
        echo "Using 'cargo' with target ${TARGET}..."
        cargo build --release --target "${TARGET}"
    else
        echo "Error: Neither 'cross' nor target '${TARGET}' is available."
        echo ""
        echo "Options:"
        echo "  1. Install cross: cargo install cross"
        echo "  2. Add target: rustup target add ${TARGET}"
        echo "  3. Run in CI with native x86_64 runner"
        exit 1
    fi
fi

BINARY_PATH="target/${TARGET}/release/k8s-stressor"
if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "Error: Binary not found at ${BINARY_PATH}"
    exit 1
fi

echo "Binary built: ${BINARY_PATH}"
file "${BINARY_PATH}"
echo ""

# Phase 2: Build Docker image
echo "=== Phase 2: Building Docker image ==="

# Create temp directory with binary for Docker context
DOCKER_CONTEXT=$(mktemp -d)
trap "rm -rf ${DOCKER_CONTEXT}" EXIT

mkdir -p "${DOCKER_CONTEXT}/target/release"
cp "${BINARY_PATH}" "${DOCKER_CONTEXT}/target/release/k8s-stressor"
cp Dockerfile.runtime "${DOCKER_CONTEXT}/Dockerfile"

# Build image
nerdctl build --platform linux/amd64 \
    -t "${IMAGE}:${TAG}" \
    -t "${IMAGE}:latest" \
    "${DOCKER_CONTEXT}"

echo ""
echo "Image built: ${IMAGE}:${TAG}"

if [[ "${SKIP_PUSH}" == "true" ]]; then
    echo ""
    echo "=== Skipping push (--skip-push specified) ==="
    exit 0
fi

# Phase 3: Push to ECR
echo ""
echo "=== Phase 3: Pushing to ECR ==="

# Authenticate with ECR
echo "Authenticating with ECR..."
aws ecr get-login-password --region "${AWS_REGION}" | \
    nerdctl login --username AWS --password-stdin "${ECR_REGISTRY}"

# Push images
echo "Pushing ${IMAGE}:${TAG}..."
nerdctl push "${IMAGE}:${TAG}"

echo "Pushing ${IMAGE}:latest..."
nerdctl push "${IMAGE}:latest"

echo ""
echo "=== Complete ==="
echo "Images pushed:"
echo "  - ${IMAGE}:${TAG}"
echo "  - ${IMAGE}:latest"
