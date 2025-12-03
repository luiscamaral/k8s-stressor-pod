#!/usr/bin/env bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-12-02
#
# Docker build script for k8s-stressor
# All compilation happens inside Docker using the multi-stage Dockerfile.
#
# Usage:
#   ./scripts/build-and-push.sh                    # Build for native arch, push to ECR
#   ./scripts/build-and-push.sh --skip-push        # Build only, don't push
#   ./scripts/build-and-push.sh --multi-arch       # Build for amd64+arm64 (may use QEMU)
#   ./scripts/build-and-push.sh --tag v1.0.0       # Use custom tag
#
# Note: Multi-arch builds use CI (GitHub Actions) for reliability.
#       Local builds default to native architecture to avoid QEMU issues.

set -euo pipefail

# Detect host architecture
case "$(uname -m)" in
    x86_64)        HOST_PLATFORM="linux/amd64" ;;
    aarch64|arm64) HOST_PLATFORM="linux/arm64" ;;
    *)             HOST_PLATFORM="linux/amd64" ;;
esac

# Configuration
ECR_REGISTRY="${ECR_REGISTRY:-506741563541.dkr.ecr.us-east-1.amazonaws.com}"
ECR_REPO="${ECR_REPO:-k8s-stressor}"
AWS_REGION="${AWS_REGION:-us-east-1}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

# Parse arguments
SKIP_PUSH=false
CUSTOM_TAG=""
MULTI_ARCH=false

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
        --multi-arch)
            MULTI_ARCH=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--skip-push] [--tag TAG] [--multi-arch]"
            exit 1
            ;;
    esac
done

# Set platforms based on flags and environment
if [[ -n "${PLATFORMS:-}" ]]; then
    BUILD_PLATFORMS="${PLATFORMS}"
elif [[ "${MULTI_ARCH}" == "true" ]]; then
    BUILD_PLATFORMS="linux/amd64,linux/arm64"
else
    BUILD_PLATFORMS="${HOST_PLATFORM}"
fi

TAG="${CUSTOM_TAG:-$VERSION}"
IMAGE="${ECR_REGISTRY}/${ECR_REPO}"

echo "=== k8s-stressor Build & Push ==="
echo "Version: ${VERSION}"
echo "Tag: ${TAG}"
echo "Host arch: ${HOST_PLATFORM}"
echo "Platforms: ${BUILD_PLATFORMS}"
echo "Image: ${IMAGE}"
echo ""

# Warn about QEMU emulation for cross-arch builds
if [[ "${BUILD_PLATFORMS}" == *","* ]] || [[ "${BUILD_PLATFORMS}" != "${HOST_PLATFORM}" ]]; then
    echo "⚠️  Warning: Building for non-native architecture may use QEMU emulation."
    echo "   Rust compilation under QEMU can be slow or fail."
    echo "   For reliable multi-arch builds, use CI (GitHub Actions)."
    echo ""
fi

echo "=== Phase 1: Building Docker image (inside Docker) ==="

# Build image using the main multi-stage Dockerfile
nerdctl build \
    --platform "${BUILD_PLATFORMS}" \
    -t "${IMAGE}:${TAG}" \
    -t "${IMAGE}:latest" \
    .

echo ""
echo "Image built: ${IMAGE}:${TAG}"

if [[ "${SKIP_PUSH}" == "true" ]]; then
    echo ""
    echo "=== Skipping push (--skip-push specified) ==="
    exit 0
fi

# Phase 2: Push to ECR
echo ""
echo "=== Phase 2: Pushing to ECR ==="

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
