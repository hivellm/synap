#!/bin/bash
# Run all k6 load tests for Synap
# 
# Prerequisites:
#   - k6 installed (https://k6.io/docs/getting-started/installation/)
#   - Synap server running on localhost:15500

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
SYNAP_URL="${SYNAP_URL:-http://localhost:15500}"
RESULTS_DIR="tests/load/results"

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check if k6 is installed
if ! command -v k6 &> /dev/null; then
    echo -e "${RED}Error: k6 is not installed${NC}"
    echo "Install from: https://k6.io/docs/getting-started/installation/"
    exit 1
fi

# Check if Synap is running
if ! curl -sf "$SYNAP_URL/health" > /dev/null; then
    echo -e "${RED}Error: Synap server not running at $SYNAP_URL${NC}"
    echo "Start server: ./target/release/synap-server --config config.yml"
    exit 1
fi

echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}Synap Load Testing Suite${NC}"
echo -e "${GREEN}======================================${NC}"
echo ""
echo "Server: $SYNAP_URL"
echo "Results: $RESULTS_DIR"
echo ""

# Function to run test
run_test() {
    local test_name=$1
    local test_file=$2
    
    echo -e "${YELLOW}Running: $test_name${NC}"
    echo "File: $test_file"
    echo "Started: $(date '+%Y-%m-%d %H:%M:%S')"
    echo ""
    
    k6 run "$test_file" --out json="$RESULTS_DIR/${test_name}-raw.json"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $test_name completed${NC}"
    else
        echo -e "${RED}✗ $test_name failed${NC}"
    fi
    
    echo ""
    echo "----------------------------------------"
    echo ""
}

# Run tests
echo "Starting load tests..."
echo ""

# Test 1: KV Operations
run_test "kv-operations" "tests/load/kv-operations.js"

# Test 2: Queue Operations
run_test "queue-operations" "tests/load/queue-operations.js"

# Test 3: Mixed Workload
run_test "mixed-workload" "tests/load/mixed-workload.js"

# Test 4: Stress Test (finds max throughput)
run_test "stress-test" "tests/load/stress-test.js"

echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}All Tests Complete!${NC}"
echo -e "${GREEN}======================================${NC}"
echo ""
echo "Results saved to: $RESULTS_DIR"
echo ""
echo "View results:"
echo "  cat $RESULTS_DIR/stress-test-report.txt"
echo "  cat $RESULTS_DIR/mixed-workload-report.txt"
echo ""
echo "Generate report:"
echo "  ./tests/load/generate-report.sh"
echo ""

