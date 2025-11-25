# Test problematic endpoints
$baseUrl = "http://localhost:15500"

Write-Host "`n=== Testing Problematic Endpoints ===" -ForegroundColor Cyan

# Test COPY
Write-Host "`n1. Testing COPY..." -ForegroundColor Yellow
try {
    $body = @{destination="test_copy_dest"} | ConvertTo-Json
    $response = Invoke-WebRequest -Uri "$baseUrl/key/mcp_rename_dest/copy" -Method POST `
        -Headers @{"Content-Type"="application/json"} -Body $body -UseBasicParsing
    Write-Host "  Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
        $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $responseBody = $reader.ReadToEnd()
        Write-Host "  Response Body: $responseBody" -ForegroundColor Red
    }
}

# Test WATCH
Write-Host "`n2. Testing WATCH..." -ForegroundColor Yellow
try {
    $body = @{keys=@("test_key")} | ConvertTo-Json -Depth 2
    $response = Invoke-WebRequest -Uri "$baseUrl/transaction/watch" -Method POST `
        -Headers @{"Content-Type"="application/json"} -Body $body -UseBasicParsing
    Write-Host "  Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
        $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $responseBody = $reader.ReadToEnd()
        Write-Host "  Response Body: $responseBody" -ForegroundColor Red
    }
}

# Test UNWATCH
Write-Host "`n3. Testing UNWATCH..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "$baseUrl/transaction/unwatch" -Method POST `
        -Headers @{"Content-Type"="application/json"} -UseBasicParsing
    Write-Host "  Status: $($response.StatusCode)" -ForegroundColor Green
    Write-Host "  Response: $($response.Content)" -ForegroundColor Green
} catch {
    Write-Host "  Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $statusCode = [int]$_.Exception.Response.StatusCode
        Write-Host "  Status Code: $statusCode" -ForegroundColor Red
        $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $responseBody = $reader.ReadToEnd()
        Write-Host "  Response Body: $responseBody" -ForegroundColor Red
    }
}

