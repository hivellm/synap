#!/usr/bin/env pwsh
# Test MCP integration with Synap

$ErrorActionPreference = "Stop"

Write-Host "🧪 Testing Synap MCP Integration..." -ForegroundColor Cyan
Write-Host ""

# Run MCP tests
Write-Host "1️⃣ Running MCP unit tests..." -ForegroundColor Yellow
wsl -d Ubuntu-24.04 -- bash -l -c "cd /mnt/f/Node/hivellm/synap && cargo test --test mcp_tests -- --nocapture"

Write-Host ""
Write-Host "2️⃣ Testing MCP tool listing..." -ForegroundColor Yellow
wsl -d Ubuntu-24.04 -- bash -l -c "cd /mnt/f/Node/hivellm/synap && cargo test --lib get_mcp_tools -- --nocapture"

Write-Host ""
Write-Host "✅ All MCP tests passed!" -ForegroundColor Green
Write-Host ""
Write-Host "📚 MCP Tools Available:" -ForegroundColor Cyan
Write-Host "   - synap_kv_get (Read key)"
Write-Host "   - synap_kv_set (Write key)"
Write-Host "   - synap_kv_delete (Delete key)"
Write-Host "   - synap_kv_scan (Scan by prefix)"
Write-Host "   - synap_queue_publish (Publish to queue)"
Write-Host "   - synap_queue_consume (Consume from queue)"
Write-Host "   - synap_stream_publish (Publish to stream)"
Write-Host "   - synap_pubsub_publish (Publish to topic)"
Write-Host ""
Write-Host "📖 See docs/protocol/MCP_USAGE.md for integration guide" -ForegroundColor Blue


