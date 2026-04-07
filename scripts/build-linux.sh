#!/bin/bash
# Build Linux DEB Package
# Usage: ./scripts/build-linux.sh [version]

set -e

VERSION="${1:-0.1.0}"

echo "Building Synap DEB Package v$VERSION"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Check prerequisites
echo -e "\n${YELLOW}Checking prerequisites...${NC}"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo not found. Install from https://rustup.rs/${NC}"
    exit 1
fi

if ! command -v cargo-deb &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-deb...${NC}"
    cargo install cargo-deb
fi

# Check build dependencies
echo -e "${YELLOW}Checking build dependencies...${NC}"
if ! dpkg -l | grep -q build-essential; then
    echo -e "${YELLOW}Installing build-essential...${NC}"
    sudo apt-get update
    sudo apt-get install -y build-essential dpkg-dev
fi

# Clean previous builds
echo -e "\n${YELLOW}Cleaning previous builds...${NC}"
rm -f target/release/synap-server
rm -rf target/debian

# Build release binary
echo -e "\n${YELLOW}Building release binary...${NC}"
cargo build --release --features full

# Build DEB package
echo -e "\n${YELLOW}Building DEB package...${NC}"
cargo deb --no-build

# Find generated DEB
DEB_FILE=$(find target/debian -name "*.deb" | head -n 1)

if [ -f "$DEB_FILE" ]; then
    echo -e "\n${GREEN}Success! DEB package created:${NC}"
    echo -e "  ${CYAN}$DEB_FILE${NC}"
    
    # Show file info
    SIZE=$(du -h "$DEB_FILE" | cut -f1)
    echo -e "\nFile size: $SIZE"
    
    # Show package info
    echo -e "\n${YELLOW}Package information:${NC}"
    dpkg-deb -I "$DEB_FILE" | grep -E "Package:|Version:|Architecture:|Maintainer:|Description:"
    
    # Installation instructions
    echo -e "\n${YELLOW}Installation:${NC}"
    echo -e "  ${CYAN}sudo dpkg -i $DEB_FILE${NC}"
    echo -e "  or"
    echo -e "  ${CYAN}sudo apt-get install ./$DEB_FILE${NC}"
    
    # Optional: Test installation
    if [ "$2" == "--test" ]; then
        echo -e "\n${YELLOW}Testing installation...${NC}"
        sudo dpkg -i "$DEB_FILE"
        systemctl status synap
        sudo dpkg -r synap
    fi
else
    echo -e "${RED}Error: DEB file not found${NC}"
    exit 1
fi

