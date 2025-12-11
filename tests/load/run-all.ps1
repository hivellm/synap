# Run all k6 load tests for Synap (PowerShell)
# 
# Prerequisites:
#   - k6 installed (choco install k6 or https://k6.io/docs/getting-started/installation/)
#   - Synap server running on localhost:15500

$ErrorActionPreference = "Stop"

# Configuration
$SYNAP_URL = if ($env:SYNAP_URL) { $env:SYNAP_URL } else { "http://localhost:15500" }
$RESULTS_DIR = "tests\load\results"

# Create results directory
New-Item -ItemType Directory -Force -Path $RESULTS_DIR | Out-Null

# Check if k6 is installed
if (-not (Get-Command k6 -ErrorAction SilentlyContinue)) {
    Write-Host "Error: k6 is not installed" -ForegroundColor Red
    Write-Host "Install from: https://k6.io/docs/getting-started/installation/" -ForegroundColor Yellow
    exit 1
}

# Check if Synap is running
try {
    $health = Invoke-WebRequest -Uri "$SYNAP_URL/health" -UseBasicParsing -TimeoutSec 5
} catch {
    Write-Host "Error: Synap server not running at $SYNAP_URL" -ForegroundColor Red
    Write-Host "Start server: .\target\release\synap-server.exe --config config.yml" -ForegroundColor Yellow
    exit 1
}

Write-Host "======================================" -ForegroundColor Green
Write-Host "Synap Load Testing Suite" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Green
Write-Host ""
Write-Host "Server: $SYNAP_URL"
Write-Host "Results: $RESULTS_DIR"
Write-Host ""

function Run-Test {
    param(
        [string]$TestName,
        [string]$TestFile
    )
    
    Write-Host "Running: $TestName" -ForegroundColor Yellow
    Write-Host "File: $TestFile"
    Write-Host "Started: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
    Write-Host ""
    
    k6 run $TestFile --out "json=$RESULTS_DIR\${TestName}-raw.json"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ $TestName completed" -ForegroundColor Green
    } else {
        Write-Host "✗ $TestName failed" -ForegroundColor Red
    }
    
    Write-Host ""
    Write-Host "----------------------------------------"
    Write-Host ""
}

# Run tests
Write-Host "Starting load tests..."
Write-Host ""

# Test 1: KV Operations
Run-Test "kv-operations" "tests\load\kv-operations.js"

# Test 2: Queue Operations
Run-Test "queue-operations" "tests\load\queue-operations.js"

# Test 3: Mixed Workload
Run-Test "mixed-workload" "tests\load\mixed-workload.js"

# Test 4: Stress Test
Run-Test "stress-test" "tests\load\stress-test.js"

Write-Host "======================================" -ForegroundColor Green
Write-Host "All Tests Complete!" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Green
Write-Host ""
Write-Host "Results saved to: $RESULTS_DIR"
Write-Host ""
Write-Host "View results:"
Write-Host "  type $RESULTS_DIR\stress-test-report.txt"
Write-Host "  type $RESULTS_DIR\mixed-workload-report.txt"
Write-Host ""

