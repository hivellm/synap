#!/bin/bash
# ============================================================================
# Synap Docker Publish Script
# ============================================================================
#
# This script builds and publishes Synap Docker images to DockerHub
# Supports multi-arch builds (AMD64 + ARM64)
#
# Usage:
#   ./scripts/docker-publish.sh [version] [--no-build] [--no-push]
#
# Examples:
#   ./scripts/docker-publish.sh           # Build and push latest
#   ./scripts/docker-publish.sh 0.9.0      # Build and push specific version
#   ./scripts/docker-publish.sh 0.9.0 --no-build  # Only push (skip build)
#   ./scripts/docker-publish.sh 0.9.0 --no-push   # Only build (skip push)
#
# Requirements:
#   - Docker with buildx enabled
#   - DockerHub credentials (DOCKER_USERNAME and DOCKER_PASSWORD env vars)
#
# ============================================================================

set -e  # Exit on error
set -u  # Exit on undefined variable

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Parse arguments
VERSION="${1:-latest}"
NO_BUILD=false
NO_PUSH=false

for arg in "$@"; do
    case $arg in
        --no-build)
            NO_BUILD=true
            shift
            ;;
        --no-push)
            NO_PUSH=true
            shift
            ;;
    esac
done

# Configuration
IMAGE_NAME="synap"
REGISTRY="${DOCKER_REGISTRY:-hivehub}"
DOCKER_USERNAME="${DOCKER_USERNAME:-}"
DOCKER_PASSWORD="${DOCKER_PASSWORD:-}"
BUILD_DATE=$(date -u +'%Y-%m-%dT%H:%M:%SZ')

# Helper functions
print_header() {
    echo -e "${BLUE}============================================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}============================================================================${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ $1${NC}"
}

print_header "Synap Docker Publish"

print_info "Image:    ${REGISTRY}/${IMAGE_NAME}"
print_info "Version:  ${VERSION}"
print_info "Registry: DockerHub"
echo ""

# Check if Dockerfile exists
if [ ! -f "Dockerfile" ]; then
    print_error "Dockerfile not found"
    print_error "Please run this script from the synap root directory"
    exit 1
fi

# Check Docker buildx
print_info "Checking Docker buildx..."
if ! docker buildx version > /dev/null 2>&1; then
    print_error "Docker buildx is not available"
    print_error "Please install Docker with buildx support"
    exit 1
fi
print_success "Docker buildx is available"

# Create buildx builder if it doesn't exist
print_info "Setting up buildx builder..."
if ! docker buildx inspect synap-builder > /dev/null 2>&1; then
    docker buildx create --name synap-builder --use > /dev/null 2>&1
else
    docker buildx use synap-builder > /dev/null 2>&1
fi
docker buildx inspect --bootstrap > /dev/null 2>&1
print_success "Buildx builder ready"

# Login to DockerHub if pushing
if [ "$NO_PUSH" = false ]; then
    if [ -z "$DOCKER_USERNAME" ] || [ -z "$DOCKER_PASSWORD" ]; then
        print_warning "DOCKER_USERNAME or DOCKER_PASSWORD not set"
        print_info "Attempting interactive login..."
        docker login
        if [ $? -ne 0 ]; then
            print_error "Docker login failed"
            exit 1
        fi
    else
        print_info "Logging in to DockerHub..."
        echo "$DOCKER_PASSWORD" | docker login --username "$DOCKER_USERNAME" --password-stdin
        if [ $? -eq 0 ]; then
            print_success "Logged in to DockerHub"
        else
            print_error "Docker login failed"
            exit 1
        fi
    fi
fi

# Enable BuildKit for cache mounts and faster builds
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Build multi-arch image
if [ "$NO_BUILD" = false ]; then
    print_info "Building multi-arch Docker image with BuildKit (linux/amd64,linux/arm64)..."
    echo ""
    
    PUSH_FLAG=""
    if [ "$NO_PUSH" = false ]; then
        PUSH_FLAG="--push"
    fi
    
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        -t "${REGISTRY}/${IMAGE_NAME}:${VERSION}" \
        -t "${REGISTRY}/${IMAGE_NAME}:latest" \
        --build-arg BUILD_DATE="$BUILD_DATE" \
        --build-arg VERSION="$VERSION" \
        $PUSH_FLAG \
        --progress=plain \
        .
    
    if [ $? -eq 0 ]; then
        echo ""
        if [ "$NO_PUSH" = true ]; then
            print_success "Docker image built successfully!"
        else
            print_success "Docker image built and pushed successfully!"
        fi
        echo ""
        print_info "Image tags:"
        echo "  - ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
        echo "  - ${REGISTRY}/${IMAGE_NAME}:latest"
    else
        echo ""
        print_error "Docker build failed"
        exit 1
    fi
else
    print_info "Skipping build (--no-build flag)"
fi

# Push to DockerHub (only if build was skipped or push was skipped during build)
if [ "$NO_PUSH" = false ]; then
    if [ "$NO_BUILD" = true ]; then
        print_info "Pushing existing images to DockerHub..."
        
        print_info "Pushing ${REGISTRY}/${IMAGE_NAME}:${VERSION}..."
        docker push "${REGISTRY}/${IMAGE_NAME}:${VERSION}"
        if [ $? -eq 0 ]; then
            print_success "Pushed ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
        else
            print_error "Failed to push ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
            exit 1
        fi
        
        print_info "Pushing ${REGISTRY}/${IMAGE_NAME}:latest..."
        docker push "${REGISTRY}/${IMAGE_NAME}:latest"
        if [ $? -eq 0 ]; then
            print_success "Pushed ${REGISTRY}/${IMAGE_NAME}:latest"
        else
            print_error "Failed to push ${REGISTRY}/${IMAGE_NAME}:latest"
            exit 1
        fi
        
        echo ""
        print_success "All images pushed successfully!"
    else
        print_info "Images already pushed during build (buildx --push)"
    fi
else
    print_info "Skipping push (--no-push flag)"
fi

echo ""
print_success "Publish completed!"
echo ""
print_info "Next steps:"
echo "  1. Verify: docker pull ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
echo "  2. Test:    docker run -p 15500:15500 ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
echo "  3. View:    https://hub.docker.com/r/${REGISTRY}/${IMAGE_NAME}"

