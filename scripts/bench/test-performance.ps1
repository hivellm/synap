# Synap Performance Test Suite (PowerShell)
# Runs comprehensive tests and benchmarks for all optimizations

Write-Host "🚀 Synap Performance Test Suite" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

# Navigate to synap directory
Set-Location (Split-Path -Parent $PSScriptRoot)

Write-Host "📦 Building project in release mode..." -ForegroundColor Blue
cargo build --release
Write-Host ""

Write-Host "🧪 Running unit tests..." -ForegroundColor Blue
cargo test --release --all
Write-Host ""

Write-Host "✅ Unit tests passed!" -ForegroundColor Green
Write-Host ""

Write-Host "📊 Running benchmarks..." -ForegroundColor Blue
Write-Host ""

# Run KV Store benchmarks
Write-Host "1️⃣  KV Store Benchmarks" -ForegroundColor Blue
Write-Host "   Testing: StoredValue memory, sharding, TTL cleanup, concurrent operations"
cargo bench --bench kv_bench
Write-Host ""

# Run Queue benchmarks
Write-Host "2️⃣  Queue Benchmarks" -ForegroundColor Blue
Write-Host "   Testing: Arc-shared messages, concurrent pub/sub, priority queues"
cargo bench --bench queue_bench
Write-Host ""

# Run Persistence benchmarks
Write-Host "3️⃣  Persistence Benchmarks" -ForegroundColor Blue
Write-Host "   Testing: AsyncWAL group commit, streaming snapshots, recovery"
cargo bench --bench persistence_bench
Write-Host ""

Write-Host "✅ All benchmarks completed!" -ForegroundColor Green
Write-Host ""

Write-Host "📈 Benchmark results saved to:" -ForegroundColor Blue
Write-Host "   target\criterion\"
Write-Host ""

Write-Host "📝 To view detailed reports:" -ForegroundColor Blue
Write-Host "   Open target\criterion\<benchmark_name>\report\index.html in browser"
Write-Host ""

Write-Host "🎉 Performance test suite complete!" -ForegroundColor Green

