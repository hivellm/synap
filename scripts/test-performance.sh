#!/bin/bash
# Synap Performance Test Suite
# Runs comprehensive tests and benchmarks for all optimizations

set -e

echo "🚀 Synap Performance Test Suite"
echo "================================="
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Navigate to synap directory
cd "$(dirname "$0")/.."

echo -e "${BLUE}📦 Building project in release mode...${NC}"
cargo build --release
echo ""

echo -e "${BLUE}🧪 Running unit tests...${NC}"
cargo test --release --all
echo ""

echo -e "${GREEN}✅ Unit tests passed!${NC}"
echo ""

echo -e "${BLUE}📊 Running benchmarks...${NC}"
echo ""

# Run KV Store benchmarks
echo -e "${BLUE}1️⃣  KV Store Benchmarks${NC}"
echo "   Testing: StoredValue memory, sharding, TTL cleanup, concurrent operations"
cargo bench --bench kv_bench
echo ""

# Run Queue benchmarks
echo -e "${BLUE}2️⃣  Queue Benchmarks${NC}"
echo "   Testing: Arc-shared messages, concurrent pub/sub, priority queues"
cargo bench --bench queue_bench
echo ""

# Run Persistence benchmarks
echo -e "${BLUE}3️⃣  Persistence Benchmarks${NC}"
echo "   Testing: AsyncWAL group commit, streaming snapshots, recovery"
cargo bench --bench persistence_bench
echo ""

echo -e "${GREEN}✅ All benchmarks completed!${NC}"
echo ""

echo -e "${BLUE}📈 Benchmark results saved to:${NC}"
echo "   target/criterion/"
echo ""

echo -e "${BLUE}📝 To view detailed reports:${NC}"
echo "   Open target/criterion/<benchmark_name>/report/index.html"
echo ""

echo -e "${GREEN}🎉 Performance test suite complete!${NC}"

