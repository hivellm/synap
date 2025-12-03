# Script para testar se as keys têm valores
param([int]$Port = 15500)

$baseUrl = "http://localhost:$Port"

Write-Host "🔍 Testando valores das keys...`n" -ForegroundColor Cyan

$testKeys = @("user:1", "user:2", "config:app", "session:abc123", "cache:products")

foreach ($key in $testKeys) {
    try {
        $response = Invoke-RestMethod -Uri "$baseUrl/kv/get/$key" -Method GET
        Write-Host "Key: $key" -ForegroundColor Yellow
        Write-Host "  Response: $($response | ConvertTo-Json -Depth 3)" -ForegroundColor Gray
        
        # Check different response formats
        if ($response.error) {
            Write-Host "  ✗ Error: $($response.error)" -ForegroundColor Red
        } elseif ($response.found) {
            $value = $response.value
            $size = if ($value -is [string]) {
                [System.Text.Encoding]::UTF8.GetByteCount($value)
            } elseif ($value -is [PSCustomObject] -or $value -is [System.Collections.IDictionary]) {
                [System.Text.Encoding]::UTF8.GetByteCount(($value | ConvertTo-Json))
            } else {
                [System.Text.Encoding]::UTF8.GetByteCount([string]$value)
            }
            Write-Host "  ✓ Found: true" -ForegroundColor Green
            Write-Host "  Value type: $($value.GetType().Name)" -ForegroundColor Gray
            Write-Host "  Value preview: $($value.ToString().Substring(0, [Math]::Min(50, $value.ToString().Length)))" -ForegroundColor Gray
            Write-Host "  Size: $size bytes" -ForegroundColor Gray
        } elseif ($response -is [string]) {
            # Direct string response (may be escaped JSON)
            $value = $response
            try {
                $parsed = $value | ConvertFrom-Json
                if ($parsed -is [string]) {
                    $parsed = $parsed | ConvertFrom-Json
                }
                $size = [System.Text.Encoding]::UTF8.GetByteCount(($parsed | ConvertTo-Json))
                Write-Host "  ✓ Found: true (parsed from string)" -ForegroundColor Green
                Write-Host "  Value type: $($parsed.GetType().Name)" -ForegroundColor Gray
                Write-Host "  Value preview: $(($parsed | ConvertTo-Json).Substring(0, [Math]::Min(50, ($parsed | ConvertTo-Json).Length)))" -ForegroundColor Gray
                Write-Host "  Size: $size bytes" -ForegroundColor Gray
            } catch {
                $size = [System.Text.Encoding]::UTF8.GetByteCount($value)
                Write-Host "  ✓ Found: true (string value)" -ForegroundColor Green
                Write-Host "  Value: $($value.Substring(0, [Math]::Min(50, $value.Length)))" -ForegroundColor Gray
                Write-Host "  Size: $size bytes" -ForegroundColor Gray
            }
        } else {
            Write-Host "  ✗ Unknown response format" -ForegroundColor Red
        }
        Write-Host ""
    } catch {
        Write-Host "  ✗ Error: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host ""
    }
}

