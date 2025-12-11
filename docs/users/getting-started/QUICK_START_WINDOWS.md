---
title: Quick Start (Windows)
module: windows-quick-start
id: quick-start-windows
order: 3
description: Windows-specific installation and quick start guide
tags: [windows, quick-start, installation, powershell]
---

# Quick Start (Windows)

Windows-specific guide for installing and using Synap.

## Installation

### Option 1: Docker Desktop (Recommended)

**Prerequisites:**
- Docker Desktop for Windows installed

**Steps:**
```powershell
# Pull image
docker pull hivellm/synap:latest

# Run container
docker run -d `
  --name synap `
  -p 15500:15500 `
  -v synap-data:/data `
  hivellm/synap:latest

# Check status
curl http://localhost:15500/health
```

### Option 2: Binary Download

```powershell
# Download from GitHub Releases
Invoke-WebRequest -Uri "https://github.com/hivellm/synap/releases/download/v0.8.1/synap-windows-x64.zip" -OutFile "synap.zip"

# Extract
Expand-Archive -Path synap.zip -DestinationPath .

# Run server
.\synap-server.exe --config config.example.yml
```

### Option 3: Build from Source

```powershell
# Install Rust
irm https://win.rustup.rs/x86_64 | iex
rustup default nightly

# Clone repository
git clone https://github.com/hivellm/synap.git
cd synap

# Build
cargo build --release

# Run
.\target\release\synap-server.exe --config config.example.yml
```

## Quick Start

### 1. Verify Installation

```powershell
# Health check
curl http://localhost:15500/health

# Or using Invoke-WebRequest
Invoke-WebRequest -Uri http://localhost:15500/health
```

### 2. Your First Key-Value Operation

```powershell
# Set a key
$body = @{
    key = "user:1"
    value = "John Doe"
    ttl = 3600
} | ConvertTo-Json

Invoke-WebRequest -Uri http://localhost:15500/kv/set `
    -Method POST `
    -ContentType "application/json" `
    -Body $body

# Get the key
Invoke-WebRequest -Uri http://localhost:15500/kv/get/user:1
```

### 3. Your First Queue Message

```powershell
# Create queue
$queueConfig = @{
    max_depth = 1000
    ack_deadline_secs = 30
} | ConvertTo-Json

Invoke-WebRequest -Uri http://localhost:15500/queue/tasks `
    -Method POST `
    -ContentType "application/json" `
    -Body $queueConfig

# Publish message
$message = @{
    payload = @(72, 101, 108, 108, 111)
    priority = 5
} | ConvertTo-Json

Invoke-WebRequest -Uri http://localhost:15500/queue/tasks/publish `
    -Method POST `
    -ContentType "application/json" `
    -Body $message
```

## Windows Service (Optional)

### Install as Service

```powershell
# Run PowerShell as Administrator
# Install service
New-Service -Name "Synap" `
    -BinaryPathName "C:\path\to\synap-server.exe --config C:\path\to\config.yml" `
    -DisplayName "Synap Server" `
    -StartupType Automatic

# Start service
Start-Service -Name "Synap"

# Check status
Get-Service -Name "Synap"
```

### Manage Service

```powershell
# Start
Start-Service -Name "Synap"

# Stop
Stop-Service -Name "Synap"

# Restart
Restart-Service -Name "Synap"

# Check status
Get-Service -Name "Synap"
```

## PowerShell Helper Functions

Create a helper script `synap.ps1`:

```powershell
# synap.ps1
$SynapHost = "http://localhost:15500"

function Get-SynapHealth {
    Invoke-RestMethod -Uri "$SynapHost/health"
}

function Set-SynapKey {
    param(
        [string]$Key,
        [string]$Value,
        [int]$TTL = 0
    )
    
    $body = @{
        key = $Key
        value = $Value
        ttl = $TTL
    } | ConvertTo-Json
    
    Invoke-RestMethod -Uri "$SynapHost/kv/set" `
        -Method POST `
        -ContentType "application/json" `
        -Body $body
}

function Get-SynapKey {
    param([string]$Key)
    
    Invoke-RestMethod -Uri "$SynapHost/kv/get/$Key"
}

# Usage
# . .\synap.ps1
# Get-SynapHealth
# Set-SynapKey -Key "test" -Value "Hello" -TTL 3600
# Get-SynapKey -Key "test"
```

## Troubleshooting

### Port Already in Use

```powershell
# Check what's using port 15500
netstat -ano | findstr :15500

# Kill process (replace PID)
taskkill /PID <PID> /F
```

### Firewall Issues

```powershell
# Allow port through firewall
New-NetFirewallRule -DisplayName "Synap Server" `
    -Direction Inbound `
    -LocalPort 15500 `
    -Protocol TCP `
    -Action Allow
```

### Docker Issues

```powershell
# Check Docker is running
docker ps

# Check logs
docker logs synap

# Restart container
docker restart synap
```

## Next Steps

1. **[First Steps](./FIRST_STEPS.md)** - Complete guide after installation
2. **[Basic KV Operations](../kv-store/BASIC.md)** - Learn key-value operations
3. **[Message Queues](../queues/CREATING.md)** - Learn about queues
4. **[Configuration Guide](../configuration/CONFIGURATION.md)** - Configure Synap

## Related Topics

- [Installation Guide](./INSTALLATION.md) - General installation
- [Docker Installation](./DOCKER.md) - Docker deployment
- [Quick Start Guide](./QUICK_START.md) - General quick start

