---
title: Service Management
module: operations
id: service-management
order: 1
description: Managing Synap as a system service
tags: [operations, service, systemd, windows-service]
---

# Service Management

Complete guide to managing Synap as a system service.

## Linux (systemd)

### Create Service File

**`/etc/systemd/system/synap.service`:**

```ini
[Unit]
Description=Synap Server
After=network.target

[Service]
Type=simple
User=synap
Group=synap
WorkingDirectory=/opt/synap
ExecStart=/opt/synap/synap-server --config /etc/synap/config.yml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### Enable and Start

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable synap

# Start service
sudo systemctl start synap

# Check status
sudo systemctl status synap
```

### Service Commands

```bash
# Start
sudo systemctl start synap

# Stop
sudo systemctl stop synap

# Restart
sudo systemctl restart synap

# Reload (if supported)
sudo systemctl reload synap

# Status
sudo systemctl status synap

# Enable (start on boot)
sudo systemctl enable synap

# Disable (don't start on boot)
sudo systemctl disable synap
```

### View Logs

```bash
# View logs
journalctl -u synap

# Follow logs
journalctl -u synap -f

# Last 100 lines
journalctl -u synap -n 100

# Since today
journalctl -u synap --since today
```

## Windows Service

### Install Service

```powershell
# Run PowerShell as Administrator
New-Service -Name "Synap" `
    -BinaryPathName "C:\Program Files\Synap\synap-server.exe --config C:\Program Files\Synap\config.yml" `
    -DisplayName "Synap Server" `
    -StartupType Automatic `
    -Description "Synap high-performance data platform"
```

### Service Commands

```powershell
# Start
Start-Service -Name "Synap"

# Stop
Stop-Service -Name "Synap"

# Restart
Restart-Service -Name "Synap"

# Status
Get-Service -Name "Synap"
```

### View Logs

```powershell
# Event Viewer
Get-EventLog -LogName Application -Source "Synap" -Newest 100
```

## Docker

### Run as Service

```yaml
# docker-compose.yml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    restart: unless-stopped
    ports:
      - "15500:15500"
    volumes:
      - ./data:/data
      - ./config.yml:/etc/synap/config.yml
```

### Service Commands

```bash
# Start
docker-compose up -d

# Stop
docker-compose stop

# Restart
docker-compose restart

# Status
docker-compose ps

# Logs
docker-compose logs -f
```

## Auto-Start Configuration

### Linux (systemd)

Service automatically starts on boot if enabled:

```bash
sudo systemctl enable synap
```

### Windows

Service automatically starts if `StartupType` is `Automatic`:

```powershell
Set-Service -Name "Synap" -StartupType Automatic
```

## Health Checks

### systemd Health Check

```ini
[Service]
ExecStart=/opt/synap/synap-server --config /etc/synap/config.yml
ExecStartPost=/usr/bin/curl -f http://localhost:15500/health || exit 1
```

### Docker Health Check

```yaml
services:
  synap:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
```

## Resource Limits

### systemd Limits

```ini
[Service]
LimitNOFILE=65536
LimitNPROC=4096
MemoryLimit=4G
```

### Docker Limits

```yaml
services:
  synap:
    deploy:
      resources:
        limits:
          memory: 4G
          cpus: '2'
```

## Related Topics

- [Monitoring](./MONITORING.md) - Monitoring and metrics
- [Troubleshooting](./TROUBLESHOOTING.md) - Common problems
- [Log Management](./LOGS.md) - Log viewing

