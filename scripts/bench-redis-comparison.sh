#!/usr/bin/env bash
# bench-redis-comparison.sh
# Starts Redis 7 via Docker, runs the Redis vs Synap comparison benchmark,
# then stops Redis. Synap must already be running on port 15500.
#
# Usage:
#   ./scripts/bench-redis-comparison.sh [--no-synap]
#
# Options:
#   --no-synap   Skip Synap health check (Redis-only run)
#
# Requirements:
#   - Docker (for Redis) OR redis-server on PATH
#   - Synap server already running: cargo run --release -- --config config.yml

set -euo pipefail

REDIS_PORT=6379
SYNAP_PORT=15500
REDIS_CONTAINER="synap-bench-redis"
OUTPUT_DIR="docs/benchmarks"
OUTPUT_FILE="$OUTPUT_DIR/latest-run.txt"
SKIP_SYNAP_CHECK=false

for arg in "$@"; do
  [[ "$arg" == "--no-synap" ]] && SKIP_SYNAP_CHECK=true
done

mkdir -p "$OUTPUT_DIR"

# ── colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()    { echo -e "${BLUE}[bench]${NC} $*"; }
success() { echo -e "${GREEN}[bench]${NC} $*"; }
warn()    { echo -e "${YELLOW}[bench]${NC} $*"; }
die()     { echo -e "${RED}[bench]${NC} $*" >&2; exit 1; }

# ── wait for a TCP port to open ───────────────────────────────────────────────
wait_for_port() {
  local host=$1 port=$2 label=$3 max=${4:-20}
  info "Waiting for $label on $host:$port ..."
  for i in $(seq 1 $max); do
    if nc -z "$host" "$port" 2>/dev/null; then
      success "$label is ready"
      return 0
    fi
    sleep 0.5
  done
  die "$label did not become ready on $host:$port after ${max} attempts"
}

# ── Redis setup ───────────────────────────────────────────────────────────────
REDIS_STARTED_BY_US=false

if nc -z 127.0.0.1 $REDIS_PORT 2>/dev/null; then
  success "Redis already running on port $REDIS_PORT"
elif command -v docker &>/dev/null; then
  info "Starting Redis 7 via Docker ..."
  docker rm -f "$REDIS_CONTAINER" 2>/dev/null || true
  docker run -d --name "$REDIS_CONTAINER" \
    -p "${REDIS_PORT}:6379" \
    --memory="256m" \
    redis:7-alpine \
    redis-server --save "" --appendonly no
  REDIS_STARTED_BY_US=true
  wait_for_port 127.0.0.1 $REDIS_PORT "Redis"
elif command -v redis-server &>/dev/null; then
  info "Starting redis-server from PATH ..."
  redis-server --daemonize yes --port $REDIS_PORT --save "" --loglevel warning
  REDIS_STARTED_BY_US=true
  wait_for_port 127.0.0.1 $REDIS_PORT "Redis"
else
  die "No Redis found. Install Docker or redis-server, or start Redis manually on port $REDIS_PORT."
fi

# ── Synap health check ────────────────────────────────────────────────────────
if [[ "$SKIP_SYNAP_CHECK" == "false" ]]; then
  if ! nc -z 127.0.0.1 $SYNAP_PORT 2>/dev/null; then
    warn "Synap not detected on port $SYNAP_PORT."
    warn "Start it with: cargo run --release -- --config config.yml"
    warn "Then re-run this script, or use --no-synap for Redis-only mode."
    [[ "$REDIS_STARTED_BY_US" == "true" ]] && docker rm -f "$REDIS_CONTAINER" 2>/dev/null || true
    exit 1
  fi
  success "Synap is ready on port $SYNAP_PORT"
fi

# ── run benchmarks ────────────────────────────────────────────────────────────
info "Building and running benchmarks (this may take a few minutes) ..."
info "Output will be saved to $OUTPUT_FILE"

cargo bench --bench redis_vs_synap --features redis-bench 2>&1 | tee "$OUTPUT_FILE"

# ── print summary ─────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════════════════════════════"
echo "  BENCHMARK SUMMARY"
echo "════════════════════════════════════════════════════════════════"
grep -E "^(redis|synap)|time:|change:" "$OUTPUT_FILE" | head -60 || true
echo "════════════════════════════════════════════════════════════════"
echo "  Full results: $OUTPUT_FILE"
echo "  Criterion HTML: target/criterion/index.html"
echo "════════════════════════════════════════════════════════════════"

# ── cleanup ───────────────────────────────────────────────────────────────────
if [[ "$REDIS_STARTED_BY_US" == "true" ]]; then
  if command -v docker &>/dev/null && docker ps -q -f name="$REDIS_CONTAINER" | grep -q .; then
    info "Stopping Redis container ..."
    docker rm -f "$REDIS_CONTAINER" >/dev/null
    success "Redis container stopped"
  elif command -v redis-cli &>/dev/null; then
    redis-cli -p $REDIS_PORT shutdown nosave 2>/dev/null || true
    success "redis-server stopped"
  fi
fi
