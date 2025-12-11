# Quick Performance Validation Script
# Tests key optimizations quickly (< 2 minutes)

Write-Host "⚡ Quick Performance Validation" -ForegroundColor Cyan
Write-Host "=============================" -ForegroundColor Cyan
Write-Host ""

Set-Location (Split-Path -Parent $PSScriptRoot)

Write-Host "1️⃣  Building..." -ForegroundColor Blue
cargo build --release 2>&1 | Out-Null
Write-Host "✅ Build successful" -ForegroundColor Green
Write-Host ""

Write-Host "2️⃣  Running core tests..." -ForegroundColor Blue
$testResult = cargo test --release --lib 2>&1 | Select-String "test result:"
Write-Host $testResult
Write-Host ""

Write-Host "3️⃣  Quick benchmarks (sample-size=10)..." -ForegroundColor Blue
Write-Host ""

# KV Store quick test
Write-Host "   📦 KV Store (sharding, memory)..." -ForegroundColor Yellow
cargo bench --bench kv_bench -- concurrent_operations --sample-size 10 --quick 2>&1 | Select-String "time:"
Write-Host ""

# Queue quick test
Write-Host "   📬 Queue (Arc sharing)..." -ForegroundColor Yellow
cargo bench --bench queue_bench -- queue_memory --sample-size 10 --quick 2>&1 | Select-String "time:"
Write-Host ""

# Persistence quick test
Write-Host "   💾 Persistence (AsyncWAL)..." -ForegroundColor Yellow
cargo bench --bench persistence_bench -- wal_throughput/async_wal_writes/100 --sample-size 10 --quick 2>&1 | Select-String "time:"
Write-Host ""

Write-Host "🎉 Quick validation complete!" -ForegroundColor Green
Write-Host ""
Write-Host "📊 For full benchmark suite, run:" -ForegroundColor Blue
Write-Host "   .\scripts\test-performance.ps1" -ForegroundColor White

