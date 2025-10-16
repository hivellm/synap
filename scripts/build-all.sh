#!/bin/bash
# Build all platform packages
# Usage: ./scripts/build-all.sh [version]

set -e

VERSION="${1:-0.1.0}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}Building Synap v$VERSION for all platforms${NC}"

# Detect platform
OS="$(uname -s)"

case "$OS" in
    Linux*)
        echo -e "\n${YELLOW}Building Linux DEB package...${NC}"
        ./scripts/build-linux.sh "$VERSION"
        ;;
    Darwin*)
        echo -e "\n${YELLOW}Building macOS package...${NC}"
        ./scripts/build-macos.sh "$VERSION"
        ;;
    CYGWIN*|MINGW*|MSYS*)
        echo -e "\n${YELLOW}Building Windows MSI installer...${NC}"
        pwsh scripts/build-windows.ps1 "$VERSION"
        ;;
    *)
        echo "Unknown operating system: $OS"
        exit 1
        ;;
esac

# Build Docker image (cross-platform)
echo -e "\n${YELLOW}Building Docker image...${NC}"
docker build -t synap:latest -t synap:$VERSION .

echo -e "\n${GREEN}All builds complete!${NC}"

