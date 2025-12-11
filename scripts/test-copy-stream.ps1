# Test COPY and STREAM endpoints specifically
$baseUrl = "http://localhost:15500"

Write-Host "`n=== Testing COPY and STREAM ===" -ForegroundColor Cyan

# Test COPY
Write-Host "`n1. Testing COPY..." -ForegroundColor Yellow
$body1 = @{key="copy_source"; value="copy_value"} | ConvertTo-Json
try {
    Invoke-WebRequest -Uri "$baseUrl/kv/set" -Method POST -Headers @{"Content-Type"="application/json"} -Body $body1 -UseBasicParsing | Out-Null
    Write-Host "  Created source key" -ForegroundColor Green
} catch {
    Write-Host "  Failed to create source: $($_.Exception.Message)" -ForegroundColor Red
}

$body2 = @{destination="copy_dest"} | ConvertTo-Json
try {
    $response = Invoke-WebRequest -Uri "$baseUrl/key/copy_source/copy" -Method POST -Headers @{"Content-Type"="application/json"} -Body $body2 -UseBasicParsing
    Write-Host "  COPY Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  COPY Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  COPY Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
        $stream = $_.Exception.Response.GetResponseStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $responseBody = $reader.ReadToEnd()
        Write-Host "  Response Body: $responseBody" -ForegroundColor Red
    }
}

# Test STREAM CREATE
Write-Host "`n2. Testing STREAM CREATE..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "$baseUrl/stream/test_stream_room" -Method POST -Headers @{"Content-Type"="application/json"} -Body "{}" -UseBasicParsing
    Write-Host "  CREATE Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  CREATE Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  CREATE Error: $($_.Exception.Message)" -ForegroundColor Red
}

# Test STREAM PUBLISH
Write-Host "`n3. Testing STREAM PUBLISH..." -ForegroundColor Yellow
$body3 = @{event="test_event"; data=@{key="value"}} | ConvertTo-Json -Depth 3
try {
    $response = Invoke-WebRequest -Uri "$baseUrl/stream/test_stream_room/publish" -Method POST -Headers @{"Content-Type"="application/json"} -Body $body3 -UseBasicParsing
    Write-Host "  PUBLISH Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  PUBLISH Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  PUBLISH Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
        $stream = $_.Exception.Response.GetResponseStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $responseBody = $reader.ReadToEnd()
        Write-Host "  Response Body: $responseBody" -ForegroundColor Red
    }
}

# Test STREAM STATS
Write-Host "`n4. Testing STREAM STATS..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "$baseUrl/stream/test_stream_room/stats" -Method GET -UseBasicParsing
    Write-Host "  STATS Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  STATS Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  STATS Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
    }
}

