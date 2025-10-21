# Synap Build Guide

## Prerequisites

- Rust 1.82+ (install via [rustup](https://rustup.rs/))
- Cargo (comes with Rust)

## Quick Start

### Build

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release
```

### Run

```bash
# Development mode
cargo run

# Production mode (after release build)
./target/release/synap-server

# With custom log level
RUST_LOG=debug cargo run
```

### Test

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_set_get

# Run integration tests
cargo test --test integration_tests

# Run with threads
cargo test -- --test-threads=1
```

### Benchmark

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench kv_set

# Save baseline for comparison
cargo bench --bench kv_bench -- --save-baseline main

# Compare with baseline
cargo bench --bench kv_bench -- --baseline main
```

### Lint

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy

# Clippy with all warnings
cargo clippy -- -D warnings
```

## Build Configurations

### Development

```bash
cargo build
```

- Fast compilation
- Debug symbols included
- No optimizations
- Assertions enabled

### Release

```bash
cargo build --release
```

- Optimized for speed
- LTO enabled
- Code stripped
- Smaller binary size

### Profile-Guided Optimization (PGO)

For maximum performance:

```bash
# 1. Build with instrumentation
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# 2. Run with typical workload
./target/release/synap-server &
# Run load tests, then stop server

# 3. Rebuild with profile data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

## Troubleshooting

### Compilation Errors

**Error**: `radix_trie` not found

```bash
cargo update
cargo build
```

**Error**: Rust version too old

```bash
rustup update
rustc --version  # Should be 1.82+
```

### Performance Issues

Run release build instead of debug:

```bash
cargo run --release
```

Enable more CPU cores for compilation:

```bash
cargo build -j8  # Use 8 cores
```

### Test Failures

Run tests sequentially if parallel execution causes issues:

```bash
cargo test -- --test-threads=1
```

## Cross-Compilation

### Linux to Windows

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### Linux ARM64

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

## Docker Build

```bash
# Build image
docker build -t synap:latest .

# Run container
docker run -p 15500:15500 synap:latest
```

## Performance Validation

After building, validate performance meets targets:

```bash
# Run benchmarks
cargo bench

# Expected results:
# - kv_set: < 1ms p95
# - kv_get: < 0.5ms p95
# - Throughput: > 10K ops/sec
```

## Clean Build

```bash
# Remove build artifacts
cargo clean

# Full rebuild
cargo build --release
```

## Binary Size Optimization

Already configured in `Cargo.toml`:

- LTO enabled
- Single codegen unit
- Stripped symbols

Result: ~5-10MB binary size (release mode)

## Continuous Integration

GitHub Actions workflow:

```yaml
- name: Build
  run: cargo build --verbose
  
- name: Test
  run: cargo test --verbose
  
- name: Lint
  run: |
    cargo fmt -- --check
    cargo clippy -- -D warnings
```

## See Also

- [Development Guide](docs/DEVELOPMENT.md)
- [Testing Guide](tests/README.md)
- [Benchmarking](benches/README.md)

