# Development Guide

## Prerequisites

### Required Tools

- **Rust**: 1.75+ (Edition 2024 support)
- **Cargo**: Latest version
- **Git**: For version control

### Optional Tools

- **Docker**: For containerized development
- **docker-compose**: For multi-node testing
- **wrk**: HTTP benchmarking
- **websocat**: WebSocket testing

## Getting Started

### Clone Repository

```bash
git clone https://github.com/hivellm/synap.git
cd synap
```

### Build

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release

# With all features
cargo build --release --all-features
```

### Run

```bash
# Development mode (debug build)
cargo run

# Production mode (release build)
./target/release/synap-server

# With custom config
./target/release/synap-server --config config.yml
```

## Project Structure

```
synap/
├── Cargo.toml                 # Project manifest
├── Cargo.lock                 # Dependency lock file
├── config.yml                 # Default configuration
│
├── src/
│   ├── main.rs                # Server entry point
│   ├── lib.rs                 # Library exports
│   │
│   ├── protocol/              # Protocol layer
│   │   ├── mod.rs
│   │   ├── streamable_http.rs
│   │   ├── websocket.rs
│   │   └── envelope.rs
│   │
│   ├── core/                  # Core components
│   │   ├── mod.rs
│   │   ├── kv_store.rs
│   │   ├── queue.rs
│   │   ├── event_stream.rs
│   │   └── pubsub.rs
│   │
│   ├── replication/           # Replication system
│   │   ├── mod.rs
│   │   ├── master.rs
│   │   ├── replica.rs
│   │   └── log.rs
│   │
│   ├── server/                # HTTP server
│   │   ├── mod.rs
│   │   ├── handlers.rs
│   │   └── router.rs
│   │
│   └── utils/                 # Utilities
│       ├── mod.rs
│       ├── error.rs
│       └── metrics.rs
│
├── tests/                     # Integration tests
│   ├── kv_tests.rs
│   ├── queue_tests.rs
│   ├── stream_tests.rs
│   └── replication_tests.rs
│
├── benches/                   # Benchmarks
│   ├── kv_bench.rs
│   └── queue_bench.rs
│
├── examples/                  # Example applications
│   ├── chat.rs
│   ├── task_queue.rs
│   └── pubsub.rs
│
├── client-sdks/               # Client SDKs
│   ├── typescript/
│   ├── python/
│   └── rust/
│
└── docs/                      # Documentation
    ├── README.md
    ├── ARCHITECTURE.md
    └── ...
```

## Development Workflow

### 1. Feature Development

```bash
# Create feature branch
git checkout -b feature/my-feature

# Make changes
vim src/core/kv_store.rs

# Run tests
cargo test

# Run benchmarks
cargo bench

# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Commit
git add .
git commit -m "feat: add new feature"
```

### 2. Testing

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_kv_set_get() {
        let store = KVStore::new(KVConfig::default());
        
        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        let result = store.get("key1").await.unwrap();
        
        assert!(result.is_some());
        assert_eq!(result.unwrap(), b"value1");
    }
}
```

#### Integration Tests

```rust
// tests/integration_test.rs
use synap::{Server, ClientConfig};

#[tokio::test]
async fn test_full_workflow() {
    // Start server
    let server = Server::new(ServerConfig::default()).await.unwrap();
    tokio::spawn(server.start());
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let client = SynapClient::connect("http://localhost:15500").await.unwrap();
    
    // Test operations
    client.kv_set("test", "value", None).await.unwrap();
    let result = client.kv_get::<String>("test").await.unwrap();
    assert_eq!(result.value.unwrap(), "value");
}
```

#### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_kv_set_get

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test integration_test
```

### 3. Benchmarking

```bash
# Run all benchmarks
cargo bench

# Specific benchmark
cargo bench kv_bench

# Compare with baseline
cargo bench --bench kv_bench -- --save-baseline main
git checkout feature/optimization
cargo bench --bench kv_bench -- --baseline main
```

### 4. Code Quality

#### Formatting

```bash
# Format all code
cargo fmt

# Check formatting
cargo fmt -- --check
```

#### Linting

```bash
# Run clippy
cargo clippy -- -D warnings

# Pedantic lints
cargo clippy -- -W clippy::pedantic

# Fix auto-fixable issues
cargo clippy --fix
```

#### Security Audit

```bash
# Install cargo-audit
cargo install cargo-audit

# Run audit
cargo audit
```

## Debugging

### Logging

```rust
use tracing::{info, debug, warn, error};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    info!("Server starting...");
    debug!("Config loaded: {:?}", config);
}
```

### Running with Debug Logs

```bash
RUST_LOG=debug cargo run
RUST_LOG=synap=trace cargo run  # Very verbose
```

### GDB Debugging

```bash
# Build with debug symbols
cargo build

# Debug with GDB
rust-gdb ./target/debug/synap-server

# Set breakpoint
(gdb) break src/core/kv_store.rs:42
(gdb) run
```

### Tokio Console

```rust
// Add to Cargo.toml
console-subscriber = "0.2"

// In main.rs
#[tokio::main]
async fn main() {
    console_subscriber::init();
    // Server code
}
```

```bash
# Run server
cargo run

# In another terminal, run tokio-console
tokio-console http://127.0.0.1:6669
```

## Hot Reload Development

### Watch for Changes

```bash
# Install cargo-watch
cargo install cargo-watch

# Auto-rebuild on changes
cargo watch -x 'run'

# With tests
cargo watch -x 'test' -x 'run'
```

## Docker Development

### Dockerfile.dev

```dockerfile
FROM rust:1.75

WORKDIR /app

# Install development tools
RUN cargo install cargo-watch

# Copy source
COPY . .

# Development command
CMD ["cargo", "watch", "-x", "run"]
```

### docker-compose.yml

```yaml
version: '3.8'

services:
  synap-master:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "15500:15500"
      - "15501:15501"  # Replication port
    environment:
      - RUST_LOG=debug
      - SYNAP_ROLE=master
    volumes:
      - ./src:/app/src
      - ./config-master.yml:/app/config.yml
  
  synap-replica-1:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "15510:15500"
    environment:
      - RUST_LOG=debug
      - SYNAP_ROLE=replica
    volumes:
      - ./src:/app/src
      - ./config-replica.yml:/app/config.yml
    depends_on:
      - synap-master
```

## Testing Multi-Node Setup

### Start Cluster

```bash
# Start master + 2 replicas
docker-compose up

# Or manually
# Terminal 1: Master
./target/release/synap-server --config config-master.yml

# Terminal 2: Replica 1
./target/release/synap-server --config config-replica-1.yml

# Terminal 3: Replica 2
./target/release/synap-server --config config-replica-2.yml
```

### Test Replication

```bash
# Write to master
curl -X POST http://localhost:15500/api/v1/command \
  -d '{"command": "kv.set", "payload": {"key": "test", "value": "hello"}}'

# Read from replica (after ~10ms)
curl -X POST http://localhost:15510/api/v1/command \
  -d '{"command": "kv.get", "payload": {"key": "test"}}'
```

## Code Style

### Rust Formatting

```rust
// Use Rust standard style (enforced by cargo fmt)

// Good
fn process_message(msg: &Message) -> Result<Response> {
    let data = msg.parse()?;
    Ok(Response::new(data))
}

// Follow naming conventions
struct KVStore {}           // PascalCase for types
fn kv_get() {}             // snake_case for functions
const MAX_SIZE: usize = 10; // SCREAMING_SNAKE for constants
```

### Documentation Comments

```rust
/// Stores a key-value pair in the radix tree.
///
/// # Arguments
///
/// * `key` - The key name
/// * `value` - The value to store
/// * `ttl` - Optional time-to-live in seconds
///
/// # Returns
///
/// Returns `Ok(SetResult)` on success, or `Err(KVError)` on failure.
///
/// # Example
///
/// ```
/// let result = store.set("user:1", b"Alice", Some(3600)).await?;
/// assert!(result.success);
/// ```
pub async fn set(
    &self,
    key: &str,
    value: Vec<u8>,
    ttl: Option<u64>
) -> Result<SetResult> {
    // Implementation
}
```

## Contributing

### Pull Request Process

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Make changes and add tests
4. Run test suite (`cargo test`)
5. Run clippy (`cargo clippy`)
6. Format code (`cargo fmt`)
7. Commit changes (`git commit -m 'feat: add amazing feature'`)
8. Push to branch (`git push origin feature/amazing-feature`)
9. Open Pull Request

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding tests
- `chore`: Build/tooling changes

**Example**:
```
feat(queue): add priority queue support

Implement priority-based message ordering in queue system.
Messages with higher priority (0-9) are consumed first.

Closes #123
```

## Code Review Checklist

### For Authors
- [ ] Tests added for new functionality
- [ ] Documentation updated
- [ ] No compiler warnings
- [ ] Clippy passes
- [ ] Benchmarks show no regression
- [ ] Error handling is comprehensive

### For Reviewers
- [ ] Code follows Rust idioms
- [ ] No unsafe code without justification
- [ ] Thread safety is correct
- [ ] Error types are appropriate
- [ ] Performance considerations addressed

## Debugging Tips

### Common Issues

**Issue**: Compilation errors with async/await

**Solution**: Ensure `#[tokio::main]` on main function:
```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Code
}
```

**Issue**: Borrow checker errors

**Solution**: Use `Arc` for shared ownership:
```rust
let data = Arc::new(RwLock::new(HashMap::new()));
let data_clone = Arc::clone(&data);
tokio::spawn(async move {
    // Use data_clone
});
```

**Issue**: Slow compilation

**Solution**: Use `sccache` or `mold` linker:
```toml
# .cargo/config.toml
[build]
rustc-wrapper = "sccache"

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
- [CONFIGURATION.md](CONFIGURATION.md) - Configuration reference
- [TESTING](../tests/) - Test suite
- [EXAMPLES](../examples/) - Example applications

