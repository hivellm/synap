#!/bin/bash
# Build macOS Package and Homebrew Formula
# Usage: ./scripts/build-macos.sh [version]

set -e

VERSION="${1:-0.1.0}"

echo "Building Synap macOS Package v$VERSION"

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

# Clean previous builds
echo -e "\n${YELLOW}Cleaning previous builds...${NC}"
rm -f target/release/synap-server
rm -f synap-macos-*.tar.gz

# Build release binary
echo -e "\n${YELLOW}Building release binary...${NC}"
cargo build --release --features full

# Create tarball
echo -e "\n${YELLOW}Creating distribution tarball...${NC}"
ARCH=$(uname -m)
TARBALL="synap-macos-${ARCH}-${VERSION}.tar.gz"

tar czf "$TARBALL" \
    -C target/release synap-server \
    -C ../.. README.md LICENSE config.example.yml

echo -e "${GREEN}Tarball created: ${CYAN}$TARBALL${NC}"

# Generate SHA256
SHA256=$(shasum -a 256 "$TARBALL" | cut -d ' ' -f 1)
echo -e "SHA256: ${CYAN}$SHA256${NC}"

# Generate Homebrew formula
echo -e "\n${YELLOW}Generating Homebrew formula...${NC}"

cat > Formula/synap.rb << EOF
class Synap < Formula
  desc "High-performance in-memory key-value store and message broker"
  homepage "https://github.com/hivellm/synap"
  url "https://github.com/hivellm/synap/releases/download/v${VERSION}/${TARBALL}"
  sha256 "${SHA256}"
  license "MIT"
  head "https://github.com/hivellm/synap.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
    
    # Install config
    etc.install "config.example.yml" => "synap/config.yml"
    
    # Create data directory
    (var/"lib/synap").mkpath
    (var/"log/synap").mkpath
  end

  service do
    run [opt_bin/"synap-server", "--config", etc/"synap/config.yml"]
    keep_alive true
    log_path var/"log/synap/synap.log"
    error_log_path var/"log/synap/synap-error.log"
  end

  test do
    system "#{bin}/synap-server", "--version"
  end
end
EOF

echo -e "${GREEN}Homebrew formula created: ${CYAN}Formula/synap.rb${NC}"

# Build instructions
echo -e "\n${YELLOW}Build complete!${NC}"
echo -e "\n${YELLOW}Distribution files:${NC}"
echo -e "  Tarball: ${CYAN}$TARBALL${NC}"
echo -e "  Formula: ${CYAN}Formula/synap.rb${NC}"

echo -e "\n${YELLOW}Next steps:${NC}"
echo -e "  1. Upload tarball to GitHub releases"
echo -e "  2. Update Homebrew tap repository"
echo -e "  3. Test installation: ${CYAN}brew install --build-from-source Formula/synap.rb${NC}"

# Optional: Test build from formula
if [ "$2" == "--test" ]; then
    echo -e "\n${YELLOW}Testing Homebrew installation...${NC}"
    brew install --build-from-source Formula/synap.rb
    synap-server --version
    brew uninstall synap
fi

