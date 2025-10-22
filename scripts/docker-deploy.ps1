# ============================================================================
# Synap Docker Deployment Script (PowerShell)
# ============================================================================
#
# This script manages the Synap replication cluster using Docker Compose
#
# Usage:
#   .\scripts\docker-deploy.ps1 [command]
#
# Commands:
#   start     - Start the cluster (1 master + 3 replicas)
#   stop      - Stop the cluster
#   restart   - Restart the cluster
#   status    - Show cluster status
#   logs      - Show logs (all nodes)
#   health    - Check health of all nodes
#   clean     - Stop and remove all data (DANGER!)
#
# Examples:
#   .\scripts\docker-deploy.ps1 start
#   .\scripts\docker-deploy.ps1 logs
#   .\scripts\docker-deploy.ps1 health
#
# ============================================================================

param(
    [Parameter(Position=0)]
    [string]$Command = "help"
)

# Configuration
$ComposeFile = "docker-compose.yml"
$MasterPort = 15500
$Replica1Port = 15510
$Replica2Port = 15520
$Replica3Port = 15530

# Helper functions
function Print-Header {
    param([string]$Message)
    Write-Host "============================================================================" -ForegroundColor Blue
    Write-Host $Message -ForegroundColor Blue
    Write-Host "============================================================================" -ForegroundColor Blue
    Write-Host ""
}

function Print-Success {
    param([string]$Message)
    Write-Host "✓ $Message" -ForegroundColor Green
}

function Print-Error {
    param([string]$Message)
    Write-Host "✗ $Message" -ForegroundColor Red
}

function Print-Warning {
    param([string]$Message)
    Write-Host "⚠ $Message" -ForegroundColor Yellow
}

function Print-Info {
    param([string]$Message)
    Write-Host "ℹ $Message" -ForegroundColor Cyan
}

function Check-Health {
    param(
        [string]$Name,
        [int]$Port
    )
    
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:${Port}/health" -UseBasicParsing -ErrorAction Stop
        if ($response.StatusCode -eq 200) {
            Print-Success "$Name is healthy (port $Port)"
            return $true
        }
    } catch {
        Print-Error "$Name is unhealthy (port $Port)"
        return $false
    }
}

# Commands
function Cmd-Start {
    Print-Header "Starting Synap Replication Cluster"
    
    if (-not (Test-Path $ComposeFile)) {
        Print-Error "docker-compose.yml not found"
        exit 1
    }
    
    Print-Info "Building images..."
    docker-compose build
    
    Print-Info "Starting services..."
    docker-compose up -d
    
    Write-Host ""
    Print-Info "Waiting for services to be healthy..."
    Start-Sleep -Seconds 5
    
    Write-Host ""
    Print-Success "Cluster started!"
    Write-Host ""
    Cmd-Status
}

function Cmd-Stop {
    Print-Header "Stopping Synap Replication Cluster"
    
    docker-compose down
    
    Write-Host ""
    Print-Success "Cluster stopped"
}

function Cmd-Restart {
    Print-Header "Restarting Synap Replication Cluster"
    
    Cmd-Stop
    Start-Sleep -Seconds 2
    Cmd-Start
}

function Cmd-Status {
    Print-Header "Synap Cluster Status"
    
    docker-compose ps
    
    Write-Host ""
    Print-Info "Service endpoints:"
    Write-Host "  Master (write):   http://localhost:$MasterPort"
    Write-Host "  Replica 1 (read): http://localhost:$Replica1Port"
    Write-Host "  Replica 2 (read): http://localhost:$Replica2Port"
    Write-Host "  Replica 3 (read): http://localhost:$Replica3Port"
}

function Cmd-Logs {
    Print-Header "Synap Cluster Logs"
    
    docker-compose logs -f
}

function Cmd-Health {
    Print-Header "Synap Cluster Health Check"
    
    Write-Host "Checking node health..."
    Write-Host ""
    
    Check-Health "Master" $MasterPort
    Check-Health "Replica 1" $Replica1Port
    Check-Health "Replica 2" $Replica2Port
    Check-Health "Replica 3" $Replica3Port
    
    Write-Host ""
    Print-Info "Checking replication status..."
    Write-Host ""
    
    # Check master replication status
    Write-Host "Master replication status:"
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:${MasterPort}/health/replication" -ErrorAction Stop
        $response | ConvertTo-Json -Depth 10
    } catch {
        Write-Host "Not available"
    }
    Write-Host ""
    
    # Check replica 1 status
    Write-Host "Replica 1 replication status:"
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:${Replica1Port}/health/replication" -ErrorAction Stop
        $response | ConvertTo-Json -Depth 10
    } catch {
        Write-Host "Not available"
    }
}

function Cmd-Clean {
    Print-Header "Clean Synap Cluster (DANGER!)"
    
    Print-Warning "This will stop all containers and DELETE ALL DATA!"
    $confirmation = Read-Host "Are you sure? (yes/no)"
    
    if ($confirmation -eq "yes") {
        docker-compose down -v
        Print-Success "Cluster stopped and all data removed"
    } else {
        Print-Info "Operation cancelled"
    }
}

function Show-Help {
    Print-Header "Synap Docker Deployment"
    Write-Host "Usage: .\docker-deploy.ps1 [command]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  start     - Start the cluster (1 master + 3 replicas)"
    Write-Host "  stop      - Stop the cluster"
    Write-Host "  restart   - Restart the cluster"
    Write-Host "  status    - Show cluster status"
    Write-Host "  logs      - Show logs (all nodes)"
    Write-Host "  health    - Check health of all nodes"
    Write-Host "  clean     - Stop and remove all data (DANGER!)"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\docker-deploy.ps1 start"
    Write-Host "  .\docker-deploy.ps1 logs"
    Write-Host "  .\docker-deploy.ps1 health"
}

# Main
switch ($Command.ToLower()) {
    "start" { Cmd-Start }
    "stop" { Cmd-Stop }
    "restart" { Cmd-Restart }
    "status" { Cmd-Status }
    "logs" { Cmd-Logs }
    "health" { Cmd-Health }
    "clean" { Cmd-Clean }
    default { Show-Help }
}

