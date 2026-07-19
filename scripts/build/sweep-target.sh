#!/usr/bin/env bash
#
# sweep-target.sh — bound the Cargo `target/` directory size.
#
# Cargo never garbage-collects target/: stale object files, incremental caches,
# and rlibs from old dependency versions accumulate until something deletes them.
# This wraps `cargo-sweep` to remove artifacts not accessed in N days WITHOUT
# breaking incrementality — the hot set (recently touched artifacts) stays.
#
# Usage:
#   scripts/build/sweep-target.sh [DAYS]     # remove artifacts unused for DAYS (default 14)
#   scripts/build/sweep-target.sh --clean    # full `cargo clean` (reclaim everything)
#   scripts/build/sweep-target.sh --dry-run  # show what would be removed, delete nothing
#   SWEEP_DAYS=30 scripts/build/sweep-target.sh
#
# See docs/rust-target-hygiene.md for the full rationale.
set -euo pipefail

# Resolve repo root (one dir up from this script) so the sweep works from anywhere.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

DAYS="${SWEEP_DAYS:-14}"
MODE="sweep"

for arg in "$@"; do
  case "${arg}" in
    --clean)   MODE="clean" ;;
    --dry-run) MODE="dry-run" ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *[!0-9]*) echo "error: unrecognized argument '${arg}'" >&2; exit 2 ;;
    *) DAYS="${arg}" ;;
  esac
done

target_size() {
  if [ -d target ]; then du -sh target 2>/dev/null | cut -f1; else echo "0"; fi
}

echo "target/ size before: $(target_size)"

if [ "${MODE}" = "clean" ]; then
  echo "Running full 'cargo clean'…"
  cargo clean
  echo "target/ size after:  $(target_size)"
  exit 0
fi

# Ensure cargo-sweep is available; install on first use.
if ! cargo sweep --version >/dev/null 2>&1; then
  echo "cargo-sweep not found — installing (cargo install cargo-sweep)…"
  cargo install cargo-sweep --locked
fi

if [ "${MODE}" = "dry-run" ]; then
  echo "Dry run: artifacts unused for > ${DAYS} days (nothing will be deleted)…"
  cargo sweep --dry-run --time "${DAYS}"
else
  echo "Sweeping artifacts unused for > ${DAYS} days…"
  cargo sweep --time "${DAYS}"
fi

echo "target/ size after:  $(target_size)"
