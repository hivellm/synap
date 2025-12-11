#!/bin/bash
# ============================================================================
# Synap Docker Build Script
# ============================================================================
#
# This script builds the Synap Docker image with proper tagging
#
# Usage:
#   ./scripts/docker-build.sh [version]
#
# Examples:
#   ./scripts/docker-build.sh           # Build with 'latest' tag
#   ./scripts/docker-build.sh 0.9.0     # Build with specific version
#
# ============================================================================

set -e  # Exit on error
set -u  # Exit on undefined variable

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="synap"
REGISTRY="${DOCKER_REGISTRY:-hivehub}"
VERSION="${1:-latest}"

echo -e "${BLUE}============================================================================${NC}"
echo -e "${BLUE}Synap Docker Build${NC}"
echo -e "${BLUE}============================================================================${NC}"
echo ""
echo -e "${GREEN}Image:${NC}    ${REGISTRY}/${IMAGE_NAME}"
echo -e "${GREEN}Version:${NC}  ${VERSION}"
echo ""

# Check if Dockerfile exists
if [ ! -f "Dockerfile" ]; then
    echo -e "${RED}Error: Dockerfile not found${NC}"
    echo "Please run this script from the synap root directory"
    exit 1
fi

# Enable BuildKit for cache mounts and faster builds
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Build the image with BuildKit optimizations
echo -e "${BLUE}Building Docker image with BuildKit...${NC}"
docker build \
    --progress=plain \
    -t "${REGISTRY}/${IMAGE_NAME}:${VERSION}" \
    -t "${REGISTRY}/${IMAGE_NAME}:latest" \
    --build-arg BUILD_DATE="$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
    --build-arg VERSION="${VERSION}" \
    .

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Docker image built successfully!${NC}"
    echo ""
    echo -e "${BLUE}Image tags:${NC}"
    echo "  - ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
    echo "  - ${REGISTRY}/${IMAGE_NAME}:latest"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "  1. Test the image:    docker run -p 15500:15500 ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
    echo "  2. Push to registry:  docker push ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
    echo "  3. Deploy cluster:    docker-compose up -d"
else
    echo ""
    echo -e "${RED}✗ Docker build failed${NC}"
    exit 1
fi

