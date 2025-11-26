#!/usr/bin/env pwsh
# Script para popular o Synap com dados reais para visualizar no dashboard
# Uso: .\scripts\populate-synap-dashboard.ps1 -Url "http://localhost" -Port 8080

param(
    [string]$Url = "http://localhost",
    [int]$Port = 8080,
    [string]$ApiKey = $null
)

$ErrorActionPreference = "Stop"

$baseUrl = if ($Port) { "$Url`:$Port" } else { $Url }
$headers = @{
    "Content-Type" = "application/json"
}

if ($ApiKey) {
    $headers["Authorization"] = "Bearer $ApiKey"
}

Write-Host "🚀 Populando Synap em $baseUrl com dados de teste..." -ForegroundColor Cyan

# Função helper para fazer requisições
function Invoke-SynapRequest {
    param(
        [string]$Method,
        [string]$Endpoint,
        [object]$Body = $null
    )
    
    $uri = "$baseUrl$Endpoint"
    
    try {
        $params = @{
            Uri = $uri
            Method = $Method
            Headers = $headers
            ErrorAction = "Stop"
        }
        
        if ($Body) {
            $params["Body"] = ($Body | ConvertTo-Json -Depth 10)
        }
        
        $response = Invoke-RestMethod @params
        return @{ Success = $true; Data = $response }
    }
    catch {
        Write-Warning "Erro em $Method $Endpoint : $($_.Exception.Message)"
        return @{ Success = $false; Error = $_.Exception.Message }
    }
}

# 1. Verificar saúde do servidor
Write-Host "`n📊 Verificando saúde do servidor..." -ForegroundColor Yellow
$health = Invoke-SynapRequest -Method "GET" -Endpoint "/health"
if (-not $health.Success) {
    Write-Host "❌ Servidor não está respondendo! Verifique se o Synap está rodando." -ForegroundColor Red
    exit 1
}
Write-Host "✅ Servidor está saudável!" -ForegroundColor Green

# 2. Criar Keys KV Store
Write-Host "`n🔑 Criando keys no KV Store..." -ForegroundColor Yellow
$kvKeys = @(
    @{ key = "user:1"; value = '{"name":"João Silva","email":"joao@example.com","age":30}' },
    @{ key = "user:2"; value = '{"name":"Maria Santos","email":"maria@example.com","age":25}' },
    @{ key = "user:3"; value = '{"name":"Pedro Costa","email":"pedro@example.com","age":35}' },
    @{ key = "config:app"; value = '{"version":"1.0.0","environment":"production"}' },
    @{ key = "config:database"; value = '{"host":"localhost","port":5432}' },
    @{ key = "session:abc123"; value = '{"userId":1,"expires":1735689600}' },
    @{ key = "session:def456"; value = '{"userId":2,"expires":1735689600}' },
    @{ key = "cache:products"; value = '["prod1","prod2","prod3"]' },
    @{ key = "cache:categories"; value = '["cat1","cat2"]' },
    @{ key = "temp:upload:123"; value = "temporary data" }
)

$kvCount = 0
foreach ($item in $kvKeys) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/kv/set" -Body @{ key = $item.key; value = $item.value }
    if ($result.Success) {
        $kvCount++
        Write-Host "  ✓ Criado: $($item.key)" -ForegroundColor Gray
    }
}
Write-Host "✅ Criadas $kvCount keys no KV Store" -ForegroundColor Green

# 3. Criar algumas keys com TTL
Write-Host "`n⏰ Criando keys com TTL..." -ForegroundColor Yellow
$ttlKeys = @(
    @{ key = "temp:token:1"; value = "token123"; ttl = 3600 },
    @{ key = "temp:token:2"; value = "token456"; ttl = 1800 },
    @{ key = "cache:rate:limit"; value = "100"; ttl = 60 }
)

foreach ($item in $ttlKeys) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/kv/set" -Body @{ key = $item.key; value = $item.value; ttl = $item.ttl }
    if ($result.Success) {
        Write-Host "  ✓ Criado com TTL: $($item.key) (TTL: $($item.ttl)s)" -ForegroundColor Gray
    }
}

# 4. Criar Queues
Write-Host "`n📬 Criando queues..." -ForegroundColor Yellow
$queues = @("email-queue", "notification-queue", "task-queue", "analytics-queue", "backup-queue")

foreach ($queue in $queues) {
    # Queues são criadas automaticamente ao publicar, mas podemos tentar criar explicitamente
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/queue/$queue" -Body @{}
    if ($result.Success) {
        Write-Host "  ✓ Queue criada: $queue" -ForegroundColor Gray
    } else {
        # Se falhar, a queue será criada ao publicar
        Write-Host "  ⚠ Queue será criada ao publicar: $queue" -ForegroundColor Yellow
    }
}

# 5. Publicar mensagens nas queues
Write-Host "`n📨 Publicando mensagens nas queues..." -ForegroundColor Yellow
$messages = @(
    @{ queue = "email-queue"; message = "Send welcome email to user@example.com"; priority = 5 },
    @{ queue = "email-queue"; message = "Send password reset to admin@example.com"; priority = 8 },
    @{ queue = "email-queue"; message = "Send newsletter to subscribers"; priority = 3 },
    @{ queue = "notification-queue"; message = "User logged in: user123"; priority = 5 },
    @{ queue = "notification-queue"; message = "Order placed: order456"; priority = 7 },
    @{ queue = "task-queue"; message = "Process payment for order789"; priority = 9 },
    @{ queue = "task-queue"; message = "Generate report for Q1"; priority = 4 },
    @{ queue = "analytics-queue"; message = "Track page view: /home"; priority = 2 },
    @{ queue = "analytics-queue"; message = "Track event: button_click"; priority = 2 },
    @{ queue = "backup-queue"; message = "Backup database scheduled"; priority = 1 }
)

$msgCount = 0
foreach ($msg in $messages) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/queue/$($msg.queue)/publish" -Body @{
        message = $msg.message
        priority = $msg.priority
    }
    if ($result.Success) {
        $msgCount++
    }
}
Write-Host "✅ Publicadas $msgCount mensagens nas queues" -ForegroundColor Green

# 6. Criar Streams
Write-Host "`n🌊 Criando streams..." -ForegroundColor Yellow
$streams = @(
    @{ room = "chat:general"; partitions = 1 },
    @{ room = "chat:support"; partitions = 2 },
    @{ room = "events:user-actions"; partitions = 3 },
    @{ room = "events:system"; partitions = 1 },
    @{ room = "logs:application"; partitions = 4 }
)

foreach ($stream in $streams) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/stream/$($stream.room)" -Body @{
        partitions = $stream.partitions
    }
    if ($result.Success) {
        Write-Host "  ✓ Stream criado: $($stream.room) (partitions: $($stream.partitions))" -ForegroundColor Gray
    }
}

# 7. Publicar mensagens nos streams
Write-Host "`n📡 Publicando mensagens nos streams..." -ForegroundColor Yellow
$streamMessages = @(
    @{ room = "chat:general"; partition = 0; message = "Hello everyone!" },
    @{ room = "chat:general"; partition = 0; message = "How are you doing?" },
    @{ room = "chat:support"; partition = 0; message = "User needs help with login" },
    @{ room = "chat:support"; partition = 1; message = "Issue resolved" },
    @{ room = "events:user-actions"; partition = 0; message = '{"action":"login","userId":1}' },
    @{ room = "events:user-actions"; partition = 1; message = '{"action":"purchase","userId":2}' },
    @{ room = "events:user-actions"; partition = 2; message = '{"action":"logout","userId":1}' },
    @{ room = "events:system"; partition = 0; message = "System backup completed" },
    @{ room = "logs:application"; partition = 0; message = "INFO: Application started" },
    @{ room = "logs:application"; partition = 1; message = "WARN: High memory usage detected" },
    @{ room = "logs:application"; partition = 2; message = "ERROR: Database connection failed" },
    @{ room = "logs:application"; partition = 3; message = "INFO: Cache cleared" }
)

$streamMsgCount = 0
foreach ($msg in $streamMessages) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/stream/$($msg.room)/partitions/$($msg.partition)/publish" -Body @{
        message = $msg.message
    }
    if ($result.Success) {
        $streamMsgCount++
    }
}
Write-Host "✅ Publicadas $streamMsgCount mensagens nos streams" -ForegroundColor Green

# 8. Criar Hash structures
Write-Host "`n🗂️  Criando Hash structures..." -ForegroundColor Yellow
$hashes = @(
    @{ key = "user:profile:1"; fields = @{ name = "João"; email = "joao@example.com"; age = "30" } },
    @{ key = "user:profile:2"; fields = @{ name = "Maria"; email = "maria@example.com"; age = "25" } },
    @{ key = "product:1"; fields = @{ name = "Laptop"; price = "1299.99"; category = "Electronics" } }
)

foreach ($hash in $hashes) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/hash/$($hash.key)/mset" -Body @{ fields = $hash.fields }
    if ($result.Success) {
        Write-Host "  ✓ Hash criado: $($hash.key)" -ForegroundColor Gray
    }
}

# 9. Criar List structures
Write-Host "`n📋 Criando List structures..." -ForegroundColor Yellow
$lists = @(
    @{ key = "tasks:todo"; values = @("Task 1", "Task 2", "Task 3") },
    @{ key = "shopping:cart"; values = @("Apple", "Banana", "Orange") }
)

foreach ($list in $lists) {
    foreach ($value in $list.values) {
        $result = Invoke-SynapRequest -Method "POST" -Endpoint "/list/$($list.key)/rpush" -Body @{ value = $value }
    }
    if ($result.Success) {
        Write-Host "  ✓ List criada: $($list.key)" -ForegroundColor Gray
    }
}

# 10. Criar Set structures
Write-Host "`n🔢 Criando Set structures..." -ForegroundColor Yellow
$sets = @(
    @{ key = "tags:article:1"; values = @("tech", "programming", "rust") },
    @{ key = "tags:article:2"; values = @("design", "ui", "ux") }
)

foreach ($set in $sets) {
    foreach ($value in $set.values) {
        $result = Invoke-SynapRequest -Method "POST" -Endpoint "/set/$($set.key)/sadd" -Body @{ value = $value }
    }
    if ($result.Success) {
        Write-Host "  ✓ Set criado: $($set.key)" -ForegroundColor Gray
    }
}

# 11. Criar Sorted Set structures
Write-Host "`n📊 Criando Sorted Set structures..." -ForegroundColor Yellow
$sortedSets = @(
    @{ key = "leaderboard:game"; members = @(
        @{ member = "player1"; score = 1500 },
        @{ member = "player2"; score = 1200 },
        @{ member = "player3"; score = 1800 }
    )}
)

foreach ($sortedSet in $sortedSets) {
    foreach ($item in $sortedSet.members) {
        $result = Invoke-SynapRequest -Method "POST" -Endpoint "/sortedset/$($sortedSet.key)/zadd" -Body @{
            member = $item.member
            score = $item.score
        }
    }
    if ($result.Success) {
        Write-Host "  ✓ Sorted Set criado: $($sortedSet.key)" -ForegroundColor Gray
    }
}

# 12. Publicar em Pub/Sub topics
Write-Host "`n📢 Publicando em Pub/Sub topics..." -ForegroundColor Yellow
$pubsubTopics = @(
    @{ topic = "news:tech"; payload = "New Rust version released!" },
    @{ topic = "news:tech"; payload = "Vue 3.5 is out!" },
    @{ topic = "notifications:users"; payload = "System maintenance scheduled" },
    @{ topic = "notifications:users"; payload = "New feature available" },
    @{ topic = "alerts:system"; payload = "High CPU usage detected" }
)

$pubsubCount = 0
foreach ($item in $pubsubTopics) {
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/pubsub/$($item.topic)/publish" -Body @{
        payload = $item.payload
    }
    if ($result.Success) {
        $pubsubCount++
    }
}
Write-Host "✅ Publicadas $pubsubCount mensagens em Pub/Sub" -ForegroundColor Green

# 13. Gerar algumas operações para criar métricas
Write-Host "`n⚡ Gerando operações para criar métricas..." -ForegroundColor Yellow
$ops = 0
for ($i = 1; $i -le 50; $i++) {
    # Ler algumas keys
    $key = "user:$($i % 3 + 1)"
    $result = Invoke-SynapRequest -Method "GET" -Endpoint "/kv/get/$key"
    if ($result.Success) { $ops++ }
    
    # Criar algumas keys temporárias
    $result = Invoke-SynapRequest -Method "POST" -Endpoint "/kv/set" -Body @{ key = "temp:test:$i"; value = "test value $i" }
    if ($result.Success) { $ops++ }
    
    # Deletar algumas keys temporárias
    if ($i % 5 -eq 0) {
        $result = Invoke-SynapRequest -Method "DELETE" -Endpoint "/kv/del/temp:test:$($i-4)"
        if ($result.Success) { $ops++ }
    }
}
Write-Host "✅ Executadas $ops operações" -ForegroundColor Green

# 14. Resumo final
Write-Host "`n📈 Resumo:" -ForegroundColor Cyan
Write-Host "  • Keys KV: $kvCount" -ForegroundColor White
Write-Host "  • Queues: $($queues.Count)" -ForegroundColor White
Write-Host "  • Mensagens em Queues: $msgCount" -ForegroundColor White
Write-Host "  • Streams: $($streams.Count)" -ForegroundColor White
Write-Host "  • Mensagens em Streams: $streamMsgCount" -ForegroundColor White
Write-Host "  • Mensagens Pub/Sub: $pubsubCount" -ForegroundColor White
Write-Host "  • Operações executadas: $ops" -ForegroundColor White

Write-Host "`n✅ Dashboard populado com sucesso! Abra o Synap Desktop para visualizar." -ForegroundColor Green

