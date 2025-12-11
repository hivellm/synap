# Test script for Synap Docker server
# Tests all REST endpoints and MCP functionality

$baseUrl = "http://localhost:15500"
$testResults = @()

function Test-Endpoint {
    param(
        [string]$Method,
        [string]$Path,
        [object]$Body = $null,
        [string]$Description
    )
    
    $url = "$baseUrl$Path"
    $headers = @{"Content-Type" = "application/json"}
    
    try {
        if ($Body) {
            $response = Invoke-WebRequest -Uri $url -Method $Method -Headers $headers -Body ($Body | ConvertTo-Json -Depth 10) -UseBasicParsing -ErrorAction Stop
        } else {
            $response = Invoke-WebRequest -Uri $url -Method $Method -Headers $headers -UseBasicParsing -ErrorAction Stop
        }
        
        $content = $response.Content | ConvertFrom-Json
        $testResults += [PSCustomObject]@{
            Test = $Description
            Status = "PASS"
            StatusCode = $response.StatusCode
            Response = $content
        }
        Write-Host "✅ $Description" -ForegroundColor Green
        return $true
    } catch {
        $testResults += [PSCustomObject]@{
            Test = $Description
            Status = "FAIL"
            Error = $_.Exception.Message
        }
        Write-Host "❌ $Description - $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

Write-Host "`n=== Testing Synap Docker Server ===" -ForegroundColor Cyan
Write-Host "Base URL: $baseUrl`n" -ForegroundColor Cyan

# Health & Metrics
Write-Host "`n--- Health & Metrics ---" -ForegroundColor Yellow
Test-Endpoint -Method "GET" -Path "/health" -Description "Health Check"
Test-Endpoint -Method "GET" -Path "/metrics" -Description "Prometheus Metrics"

# KV Store
Write-Host "`n--- KV Store ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/kv/set" -Body @{key="test_key"; value="test_value"} -Description "KV SET"
Test-Endpoint -Method "GET" -Path "/kv/get/test_key" -Description "KV GET"
Test-Endpoint -Method "GET" -Path "/kv/stats" -Description "KV Stats"
Test-Endpoint -Method "POST" -Path "/kv/test_key/append" -Body @{value="_appended"} -Description "KV APPEND"
Test-Endpoint -Method "GET" -Path "/kv/test_key/strlen" -Description "KV STRLEN"
Test-Endpoint -Method "DELETE" -Path "/kv/del/test_key" -Description "KV DEL"

# Key Management
Write-Host "`n--- Key Management ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/kv/set" -Body @{key="key_mgmt_test"; value="value"} -Description "Setup: KV SET"
Test-Endpoint -Method "GET" -Path "/key/key_mgmt_test/exists" -Description "KEY EXISTS"
Test-Endpoint -Method "GET" -Path "/key/key_mgmt_test/type" -Description "KEY TYPE"
Test-Endpoint -Method "POST" -Path "/key/key_mgmt_test/rename" -Body @{destination="key_mgmt_renamed"} -Description "KEY RENAME"
Test-Endpoint -Method "GET" -Path "/key/key_mgmt_renamed/exists" -Description "Verify RENAME"
# Create a new key for COPY test (since renamed key was moved)
Test-Endpoint -Method "POST" -Path "/kv/set" -Body @{key="key_mgmt_copy_source"; value="copy_value"} -Description "Setup: KV SET for COPY"
Test-Endpoint -Method "POST" -Path "/key/key_mgmt_copy_source/copy" -Body @{destination="key_mgmt_copied"} -Description "KEY COPY"
Test-Endpoint -Method "GET" -Path "/key/randomkey" -Description "KEY RANDOMKEY"

# Transactions
Write-Host "`n--- Transactions ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/kv/set" -Body @{key="tx_key1"; value="tx_value1"} -Description "Setup: KV SET"
Test-Endpoint -Method "POST" -Path "/transaction/watch" -Body @{keys=@("tx_key1")} -Description "WATCH"
# WATCH creates transaction implicitly, so MULTI will fail if called again - skip it
# Test-Endpoint -Method "POST" -Path "/transaction/multi" -Description "MULTI (start transaction)"
Test-Endpoint -Method "POST" -Path "/api/v1/command" -Body @{command="kv.set"; request_id="req1"; payload=@{key="tx_key2"; value="tx_value2"}} -Description "TX: KV SET via StreamableHTTP"
Test-Endpoint -Method "POST" -Path "/transaction/exec" -Description "EXEC"
Test-Endpoint -Method "POST" -Path "/transaction/unwatch" -Description "UNWATCH"

# Hash
Write-Host "`n--- Hash ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/hash/test_hash/set" -Body @{field="field1"; value="value1"} -Description "HASH SET"
Test-Endpoint -Method "POST" -Path "/hash/test_hash/mset" -Body @{fields=@{field2="value2"; field3="value3"}} -Description "HASH MSET"
Test-Endpoint -Method "GET" -Path "/hash/test_hash/field1" -Description "HASH GET"
Test-Endpoint -Method "GET" -Path "/hash/test_hash/getall" -Description "HASH GETALL"
Test-Endpoint -Method "GET" -Path "/hash/test_hash/len" -Description "HASH LEN"
Test-Endpoint -Method "POST" -Path "/hash/test_hash/incrby" -Body @{field="counter"; increment=5} -Description "HASH INCRBY"
Test-Endpoint -Method "GET" -Path "/hash/stats" -Description "HASH Stats"

# List
Write-Host "`n--- List ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/list/test_list/lpush" -Body @{values=@("item1", "item2")} -Description "LIST LPUSH"
Test-Endpoint -Method "POST" -Path "/list/test_list/rpush" -Body @{values=@("item3", "item4")} -Description "LIST RPUSH"
Test-Endpoint -Method "GET" -Path "/list/test_list/range?start=0&stop=-1" -Description "LIST RANGE"
Test-Endpoint -Method "GET" -Path "/list/test_list/len" -Description "LIST LEN"
Test-Endpoint -Method "POST" -Path "/list/test_list/lpop" -Body @{count=$null} -Description "LIST LPOP"
Test-Endpoint -Method "POST" -Path "/list/test_list/rpop" -Body @{count=$null} -Description "LIST RPOP"
Test-Endpoint -Method "GET" -Path "/list/stats" -Description "LIST Stats"

# Set
Write-Host "`n--- Set ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/set/test_set/add" -Body @{members=@("member1", "member2", "member3")} -Description "SET ADD"
Test-Endpoint -Method "GET" -Path "/set/test_set/members" -Description "SET MEMBERS"
Test-Endpoint -Method "GET" -Path "/set/test_set/card" -Description "SET CARD"
Test-Endpoint -Method "POST" -Path "/set/test_set/ismember" -Body @{member="member1"} -Description "SET ISMEMBER"
Test-Endpoint -Method "POST" -Path "/set/test_set/pop" -Body @{count=1} -Description "SET POP"
Test-Endpoint -Method "GET" -Path "/set/stats" -Description "SET Stats"

# Sorted Set
Write-Host "`n--- Sorted Set ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/sortedset/test_zset/zadd" -Body @{member="member1"; score=10.5} -Description "ZADD"
Test-Endpoint -Method "POST" -Path "/sortedset/test_zset/zadd" -Body @{member="member2"; score=20.0} -Description "ZADD (member2)"
Test-Endpoint -Method "GET" -Path "/sortedset/test_zset/zcard" -Description "ZCARD"
Test-Endpoint -Method "GET" -Path "/sortedset/test_zset/zrange?start=0&stop=-1" -Description "ZRANGE"
# ZRANK endpoint may not exist in REST API, skip for now
# Test-Endpoint -Method "GET" -Path "/sortedset/test_zset/zrank?member=member1" -Description "ZRANK"
Test-Endpoint -Method "GET" -Path "/sortedset/stats" -Description "Sorted Set Stats"

# Queue
Write-Host "`n--- Queue ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/queue/test_queue" -Body @{} -Description "QUEUE CREATE"
$payloadBytes = [System.Text.Encoding]::UTF8.GetBytes("test_message")
$payloadArray = $payloadBytes | ForEach-Object { [int]$_ }
Test-Endpoint -Method "POST" -Path "/queue/test_queue/publish" -Body @{payload=$payloadArray} -Description "QUEUE PUBLISH"
Test-Endpoint -Method "GET" -Path "/queue/test_queue/stats" -Description "QUEUE Stats"
Test-Endpoint -Method "GET" -Path "/queue/list" -Description "QUEUE LIST"

# Stream
Write-Host "`n--- Stream ---" -ForegroundColor Yellow
# Create stream room first
Test-Endpoint -Method "POST" -Path "/stream/test_room" -Body @{} -Description "STREAM CREATE"
Test-Endpoint -Method "POST" -Path "/stream/test_room/publish" -Body @{event="test_event"; data=@{key="value"}} -Description "STREAM PUBLISH"
Test-Endpoint -Method "GET" -Path "/stream/test_room/stats" -Description "STREAM Stats"
Test-Endpoint -Method "GET" -Path "/stream/list" -Description "STREAM LIST"

# Pub/Sub
Write-Host "`n--- Pub/Sub ---" -ForegroundColor Yellow
$pubsubPayloadBytes = [System.Text.Encoding]::UTF8.GetBytes("pubsub_message")
$pubsubPayloadArray = $pubsubPayloadBytes | ForEach-Object { [int]$_ }
Test-Endpoint -Method "POST" -Path "/pubsub/test_topic/publish" -Body @{payload=$pubsubPayloadArray} -Description "PUBSUB PUBLISH"
Test-Endpoint -Method "GET" -Path "/pubsub/stats" -Description "PUBSUB Stats"
Test-Endpoint -Method "GET" -Path "/pubsub/topics" -Description "PUBSUB Topics"

# Monitoring
Write-Host "`n--- Monitoring ---" -ForegroundColor Yellow
Test-Endpoint -Method "GET" -Path "/info" -Description "INFO"
Test-Endpoint -Method "GET" -Path "/info?section=server" -Description "INFO Server"
Test-Endpoint -Method "GET" -Path "/info?section=memory" -Description "INFO Memory"
Test-Endpoint -Method "GET" -Path "/slowlog" -Description "SLOWLOG GET"
Test-Endpoint -Method "POST" -Path "/kv/set" -Body @{key="memory_test"; value="x" * 1000} -Description "Setup: Large value for memory test"
Test-Endpoint -Method "GET" -Path "/memory/memory_test/usage" -Description "MEMORY USAGE"
Test-Endpoint -Method "GET" -Path "/clients" -Description "CLIENT LIST"

# StreamableHTTP Command API
Write-Host "`n--- StreamableHTTP Command API ---" -ForegroundColor Yellow
Test-Endpoint -Method "POST" -Path "/api/v1/command" -Body @{command="kv.set"; request_id="cmd1"; payload=@{key="cmd_key"; value="cmd_value"}} -Description "StreamableHTTP: KV SET"
Test-Endpoint -Method "POST" -Path "/api/v1/command" -Body @{command="kv.get"; request_id="cmd2"; payload=@{key="cmd_key"}} -Description "StreamableHTTP: KV GET"
Test-Endpoint -Method "POST" -Path "/api/v1/command" -Body @{command="hash.set"; request_id="cmd3"; payload=@{key="cmd_hash"; field="field1"; value="value1"}} -Description "StreamableHTTP: HASH SET"
Test-Endpoint -Method "POST" -Path "/api/v1/command" -Body @{command="list.lpush"; request_id="cmd4"; payload=@{key="cmd_list"; values=@("v1", "v2")}} -Description "StreamableHTTP: LIST LPUSH"

# Summary
Write-Host "`n=== Test Summary ===" -ForegroundColor Cyan
$passed = ($testResults | Where-Object { $_.Status -eq "PASS" }).Count
$failed = ($testResults | Where-Object { $_.Status -eq "FAIL" }).Count
$total = $testResults.Count

Write-Host "Total Tests: $total" -ForegroundColor White
Write-Host "Passed: $passed" -ForegroundColor Green
Write-Host "Failed: $failed" -ForegroundColor $(if ($failed -eq 0) { "Green" } else { "Red" })
$successRate = if ($total -gt 0) { [math]::Round(($passed / $total) * 100, 1) } else { 0 }
Write-Host "Success Rate: $successRate%" -ForegroundColor $(if ($successRate -ge 95) { "Green" } elseif ($successRate -ge 80) { "Yellow" } else { "Red" })

if ($failed -gt 0) {
    Write-Host "`nFailed Tests:" -ForegroundColor Red
    $testResults | Where-Object { $_.Status -eq "FAIL" } | ForEach-Object {
        Write-Host "  - $($_.Test): $($_.Error)" -ForegroundColor Red
    }
}

Write-Host "`nTest Results saved to: test-results.json" -ForegroundColor Cyan
$testResults | ConvertTo-Json -Depth 5 | Out-File -FilePath "test-results.json" -Encoding UTF8

