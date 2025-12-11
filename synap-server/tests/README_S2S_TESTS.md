# S2S (Server-to-Server) Tests

## Overview

S2S tests are integration tests that require a running Synap server. These tests are excluded from CI/CD by default to avoid requiring a server instance during automated builds.

## Running S2S Tests

### Enable the Feature

To run S2S tests, you need to enable the `s2s-tests` feature:

```bash
cargo test --features s2s-tests --test transaction_s2s_tests
```

### Prerequisites

1. **Start a Synap server** before running S2S tests
2. The server should be accessible at the URL specified in the test (default: spawned test server)

### Available S2S Test Suites

- `transaction_s2s_tests.rs` - Transaction operations S2S tests
- `s2s_rest_tests.rs` - REST API S2S tests
- `s2s_streamable_tests.rs` - StreamableHTTP protocol S2S tests
- `s2s_stream_tests.rs` - Stream operations S2S tests
- `s2s_pubsub_tests.rs` - Pub/Sub operations S2S tests

### CI/CD Exclusion

By default, S2S tests are **not compiled** during CI/CD runs because:

1. They require a running server instance
2. They may have timing dependencies
3. They are meant for manual/local testing and validation

### Running All S2S Tests

```bash
# Run all S2S tests
cargo test --features s2s-tests

# Run specific S2S test suite
cargo test --features s2s-tests --test transaction_s2s_tests
cargo test --features s2s-tests --test s2s_rest_tests
cargo test --features s2s-tests --test s2s_streamable_tests
```

### Development Workflow

1. **Local Development**: Run S2S tests with `--features s2s-tests` to validate server integration
2. **CI/CD**: Tests run without the feature, ensuring fast builds without server dependencies
3. **Pre-commit**: Optionally run S2S tests before committing if you have a server available

## Test Structure

S2S tests follow this pattern:

```rust
#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_feature_name() {
    let base_url = spawn_test_server().await;
    // ... test implementation
}
```

The `#[cfg(feature = "s2s-tests")]` attribute ensures tests are only compiled when the feature is enabled.

