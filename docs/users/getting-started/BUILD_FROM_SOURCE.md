---
title: Building from Source
module: build
id: build-from-source
order: 3
description: Build Synap from source code
tags: [build, source, compilation, development]
---

# Building from Source

Complete guide for building Synap from source code.

## Prerequisites

### Required Tools

- **Rust 1.85+** (Edition 2024, nightly)
- **Cargo** (comes with Rust)
- **Git**
- **Build tools**:
  - Linux: `build-essential`, `gcc`, `pkg-config`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools or MSVC

### Install Rust

**Linux/macOS:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup default nightly
rustup component add rustfmt clippy
```

**Windows:**
```powershell
# Download and run rustup-init.exe from https://rustup.rs/
# Or use PowerShell:
irm https://win.rustup.rs/x86_64 | iex
rustup default nightly
rustup component add rustfmt clippy
```

**Verify Installation:**
```bash
rustc --version  # Should show 1.85+ (nightly)
cargo --version
```

## Clone Repository

```bash
git clone https://github.com/hivellm/synap.git
cd synap
```

## Build

### Development Build

```bash
# Build debug version
cargo build

# Binary location: target/debug/synap-server
```

### Release Build

```bash
# Build optimized version
cargo build --release

# Binary location: target/release/synap-server
```

### Build with Features

```bash
# Build with all features
cargo build --release --features full

# Build specific features
cargo build --release --features persistence,replication
```

## Run

### Development Mode

```bash
# Run server
cargo run -- --config config.example.yml

# Or use built binary
./target/debug/synap-server --config config.example.yml
```

### Release Mode

```bash
# Run optimized server
./target/release/synap-server --config config.yml
```

## Testing

### Run All Tests

```bash
# Unit and integration tests
cargo test

# With output
cargo test -- --nocapture

# Specific test
cargo test test_kv_set_get
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html
```

## Development Workflow

### Format Code

```bash
cargo fmt
```

### Lint Code

```bash
cargo clippy -- -D warnings
```

### Watch Mode

```bash
# Install cargo-watch
cargo install cargo-watch

# Watch and rebuild
cargo watch -x 'build' -x 'test'
```

## Cross-Compilation

### Linux Target

```bash
# Install target
rustup target add x86_64-unknown-linux-gnu

# Build
cargo build --release --target x86_64-unknown-linux-gnu
```

### Windows Target (from Linux)

```bash
# Install target
rustup target add x86_64-pc-windows-gnu

# Install mingw-w64
sudo apt-get install mingw-w64

# Build
cargo build --release --target x86_64-pc-windows-gnu
```

### macOS Target (from Linux)

```bash
# Install target
rustup target add x86_64-apple-darwin

# Requires macOS SDK (use osxcross)
cargo build --release --target x86_64-apple-darwin
```

## Optimization

### Release Profile

Edit `Cargo.toml`:
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = 'abort'
```

### Build Flags

```bash
# Maximum optimization
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Link-time optimization
cargo build --release --config 'profile.release.lto="fat"'
```

## Installation

### Install to System

```bash
# Build and install
cargo install --path synap-server --force

# Or install from git
cargo install --git https://github.com/hivellm/synap.git synap-server
```

### Create Package

```bash
# Create tarball
tar czf synap-$(git describe --tags).tar.gz \
  target/release/synap-server \
  target/release/synap-cli \
  config.example.yml \
  README.md
```

## Troubleshooting

### Build Errors

**Missing Dependencies:**
```bash
# Linux
sudo apt-get install build-essential pkg-config libssl-dev

# macOS
xcode-select --install
```

**Linker Errors:**
```bash
# Set linker
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
```

### Slow Builds

```bash
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache

# Use mold linker (Linux)
export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
```

## Related Topics

- [Installation Guide](./INSTALLATION.md) - General installation
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration
- [Development Guide](../../../README.md) - Development workflow

