#!/bin/bash
# ============================================================================
# Smoke tests for scripts/docker-publish.{sh,ps1}
# ============================================================================
#
# Validates the publish scripts without pushing anything:
#   1. bash / PowerShell syntax
#   2. multi-arch + attestation + cache flags are present in both variants
#   3. dry run (--no-build --no-push) exits 0
#
# Usage: ./scripts/test-docker-publish.sh
# ============================================================================

set -e
set -u

cd "$(dirname "$0")/.."

FAILURES=0

pass() { echo "  ok: $1"; }
fail() { echo "  FAIL: $1"; FAILURES=$((FAILURES + 1)); }

echo "[1/4] bash syntax"
if bash -n scripts/docker-publish.sh; then
    pass "docker-publish.sh parses"
else
    fail "docker-publish.sh has syntax errors"
fi

echo "[2/4] PowerShell syntax"
if command -v pwsh > /dev/null 2>&1; then
    if pwsh -NoProfile -Command \
        '$e = $null; [System.Management.Automation.Language.Parser]::ParseFile("scripts/docker-publish.ps1", [ref]$null, [ref]$e) | Out-Null; exit $e.Count' \
        > /dev/null 2>&1; then
        pass "docker-publish.ps1 parses"
    else
        fail "docker-publish.ps1 has syntax errors"
    fi
else
    echo "  pwsh not found on PATH — PowerShell parse check not run on this host"
fi

echo "[3/4] required buildx flags present in both variants"
for script in scripts/docker-publish.sh scripts/docker-publish.ps1; do
    for flag in \
        "--platform linux/amd64,linux/arm64" \
        "--sbom=true" \
        "--provenance=mode=max" \
        "type=registry,ref=" \
        "docker-container"; do
        if grep -qF -- "$flag" "$script"; then
            pass "$script contains '$flag'"
        else
            fail "$script missing '$flag'"
        fi
    done
done

echo "[4/4] dry run (--no-build --no-push) exits 0"
if command -v docker > /dev/null 2>&1 && docker info > /dev/null 2>&1; then
    if bash scripts/docker-publish.sh test-dry-run --no-build --no-push > /dev/null 2>&1; then
        pass "dry run exited 0"
    else
        fail "dry run exited non-zero"
    fi
else
    echo "  docker daemon not available — dry run not executed on this host"
fi

echo ""
if [ "$FAILURES" -eq 0 ]; then
    echo "All docker-publish smoke tests passed"
else
    echo "$FAILURES docker-publish smoke test(s) failed"
    exit 1
fi
