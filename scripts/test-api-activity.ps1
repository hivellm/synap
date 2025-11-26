# Script para gerar atividade na API do Synap e gerar logs
# Uso: .\scripts\test-api-activity.ps1 -Port 15500 -Duration 60 -Interval 2

param(
    [int]$Port = 15500,
    [int]$Duration = 60,  # Duração em segundos
    [int]$Interval = 2    # Intervalo entre operações em segundos
)

$baseUrl = "http://localhost:$Port"
$apiKey = $null  # Ajuste se necessário

Write-Host "🚀 Iniciando testes de atividade na API do Synap" -ForegroundColor Cyan
Write-Host "   URL: $baseUrl" -ForegroundColor Gray
Write-Host "   Duração: $Duration segundos" -ForegroundColor Gray
Write-Host "   Intervalo: $Interval segundos entre operações`n" -ForegroundColor Gray

# Função para fazer requisições
function Invoke-SynapRequest {
    param(
        [string]$Method,
        [string]$Endpoint,
        [hashtable]$Body = $null
    )
    
    $url = "$baseUrl$Endpoint"
    $headers = @{
        "Content-Type" = "application/json"
    }
    
    if ($apiKey) {
        $headers["X-API-Key"] = $apiKey
    }
    
    try {
        if ($Body) {
            $jsonBody = $Body | ConvertTo-Json -Depth 10
            $response = Invoke-RestMethod -Uri $url -Method $Method -Headers $headers -Body $jsonBody -ErrorAction Stop
        } else {
            $response = Invoke-RestMethod -Uri $url -Method $Method -Headers $headers -ErrorAction Stop
        }
        return @{ Success = $true; Data = $response }
    } catch {
        $statusCode = $_.Exception.Response.StatusCode.value__
        $errorMessage = $_.Exception.Message
        return @{ Success = $false; StatusCode = $statusCode; Error = $errorMessage }
    }
}

# Verificar se o servidor está rodando
Write-Host "🔍 Verificando conexão com o servidor..." -ForegroundColor Yellow
$healthCheck = Invoke-SynapRequest -Method "GET" -Endpoint "/health"
if (-not $healthCheck.Success) {
    Write-Host "❌ Servidor não está respondendo! Verifique se o Synap está rodando na porta $Port" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Servidor está respondendo`n" -ForegroundColor Green

$startTime = Get-Date
$endTime = $startTime.AddSeconds($Duration)
$operationCount = 0
$errorCount = 0

# Lista de operações para executar
$operations = @(
    @{ Type = "KV_SET"; Endpoint = "/kv/set"; Body = @{ key = "test:counter"; value = "0" } },
    @{ Type = "KV_GET"; Endpoint = "/kv/get/test:counter"; Body = $null },
    @{ Type = "KV_INCR"; Endpoint = "/api/v1/command"; Body = @{ command = "kv.incr"; payload = @{ key = "test:counter"; amount = 1 } } },
    @{ Type = "QUEUE_PUBLISH"; Endpoint = "/queue/test-queue/publish"; Body = @{ payload = @(1,2,3); priority = 5 } },
    @{ Type = "STREAM_PUBLISH"; Endpoint = "/stream/test-room/publish"; Body = @{ event = "test"; data = @{ message = "test" } } },
    @{ Type = "PUBSUB_PUBLISH"; Endpoint = "/pubsub/test.topic/publish"; Body = @{ payload = @{ test = "data" } } },
    @{ Type = "HASH_SET"; Endpoint = "/hash/test:hash/mset"; Body = @{ fields = @{ field1 = "value1"; field2 = "value2" } } },
    @{ Type = "LIST_PUSH"; Endpoint = "/list/test:list/rpush"; Body = @{ values = @("item1", "item2") } },
    @{ Type = "SET_ADD"; Endpoint = "/api/v1/command"; Body = @{ command = "set.sadd"; payload = @{ key = "test:set"; members = @("member1", "member2") } } },
    @{ Type = "ZSET_ADD"; Endpoint = "/api/v1/command"; Body = @{ command = "sortedset.zadd"; payload = @{ key = "test:zset"; members = @(@{ member = "player1"; score = 100 }) } } },
    @{ Type = "KV_STATS"; Endpoint = "/kv/stats"; Body = $null },
    @{ Type = "QUEUE_LIST"; Endpoint = "/queue/list"; Body = $null },
    @{ Type = "STREAM_LIST"; Endpoint = "/stream/list"; Body = $null },
    @{ Type = "HEALTH"; Endpoint = "/health"; Body = $null }
)

# Criar estruturas iniciais
Write-Host "📦 Criando estruturas iniciais..." -ForegroundColor Yellow
$initOps = @(
    @{ Endpoint = "/queue/test-queue"; Body = @{} },
    @{ Endpoint = "/stream/test-room"; Body = @{} }
)

foreach ($op in $initOps) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint $op.Endpoint -Body $op.Body
    if ($result.Success) {
        Write-Host "  ✓ Criado: $($op.Endpoint)" -ForegroundColor Gray
    }
}

Write-Host "`n🔄 Iniciando loop de operações...`n" -ForegroundColor Cyan

# Loop principal
while ((Get-Date) -lt $endTime) {
    $elapsed = [int]((Get-Date) - $startTime).TotalSeconds
    $remaining = $Duration - $elapsed
    
    if ($remaining -le 0) {
        break
    }
    
    # Selecionar operação aleatória
    $op = $operations | Get-Random
    
    # Ajustar body para operações dinâmicas
    if ($op.Type -eq "KV_SET") {
        $counter = Get-Random -Minimum 1 -Maximum 1000
        $op.Body = @{ key = "test:key:$counter"; value = "value-$counter"; ttl = 3600 }
    } elseif ($op.Type -eq "KV_GET") {
        $keyNum = Get-Random -Minimum 1 -Maximum 1000
        $op.Endpoint = "/kv/get/test:key:$keyNum"
    } elseif ($op.Type -eq "QUEUE_PUBLISH") {
        $msgId = Get-Random -Minimum 1 -Maximum 10000
        $op.Body = @{ payload = @(1,2,3,4,5); priority = Get-Random -Minimum 1 -Maximum 10 }
    } elseif ($op.Type -eq "STREAM_PUBLISH") {
        $eventId = Get-Random -Minimum 1 -Maximum 10000
        $op.Body = @{ event = "test-event"; data = @{ id = $eventId; message = "Test message $eventId" } }
    }
    
    # Executar operação
    $result = if ($op.Body) {
        if ($op.Endpoint -eq "/api/v1/command") {
            # Adicionar request_id para comandos
            $op.Body.request_id = "req-$(Get-Date -Format 'yyyyMMddHHmmss')-$(Get-Random)"
        }
        Invoke-SynapRequest -Method "POST" -Endpoint $op.Endpoint -Body $op.Body
    } else {
        Invoke-SynapRequest -Method "GET" -Endpoint $op.Endpoint
    }
    
    $operationCount++
    
    if ($result.Success) {
        Write-Host "[$elapsed/$Duration] ✓ $($op.Type) - OK" -ForegroundColor Green
    } else {
        $errorCount++
        Write-Host "[$elapsed/$Duration] ✗ $($op.Type) - Erro: $($result.StatusCode) $($result.Error)" -ForegroundColor Red
    }
    
    # Aguardar intervalo
    Start-Sleep -Seconds $Interval
}

# Estatísticas finais
Write-Host "`n📊 Estatísticas:" -ForegroundColor Cyan
Write-Host "   Total de operações: $operationCount" -ForegroundColor White
Write-Host "   Sucessos: $($operationCount - $errorCount)" -ForegroundColor Green
Write-Host "   Erros: $errorCount" -ForegroundColor $(if ($errorCount -gt 0) { "Red" } else { "Green" })
Write-Host "   Taxa de sucesso: $([math]::Round((($operationCount - $errorCount) / $operationCount) * 100, 2))%" -ForegroundColor White

# Buscar estatísticas finais
Write-Host "`n📈 Estatísticas do servidor:" -ForegroundColor Cyan
$stats = Invoke-SynapRequest -Method "POST" -Endpoint "/api/v1/command" -Body @{
    command = "admin.stats"
    request_id = "stats-$(Get-Date -Format 'yyyyMMddHHmmss')"
    payload = @{}
}

if ($stats.Success -and $stats.Data.payload) {
    $payload = $stats.Data.payload
    Write-Host "   Versão: $($payload.server.version)" -ForegroundColor White
    Write-Host "   Uptime: $($payload.server.uptime_secs) segundos" -ForegroundColor White
    Write-Host "   Clientes conectados: $($payload.server.connected_clients)" -ForegroundColor White
    Write-Host "   Total de keys: $($payload.kv.total_keys)" -ForegroundColor White
    Write-Host "   Ops/seg: $($payload.kv.operations_per_sec)" -ForegroundColor White
    Write-Host "   Memória usada: $($payload.memory.used_bytes) bytes" -ForegroundColor White
}

Write-Host "`n✅ Testes concluídos!" -ForegroundColor Green

