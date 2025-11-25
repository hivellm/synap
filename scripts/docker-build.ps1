# ============================================================================
# Synap Docker Build Script (PowerShell)
# ============================================================================
#
# This script builds the Synap Docker image with proper tagging
#
# Usage:
#   .\scripts\docker-build.ps1 [version]
#
# Examples:
#   .\scripts\docker-build.ps1           # Build with 'latest' tag
#   .\scripts\docker-build.ps1 0.3.0     # Build with specific version
#
# ============================================================================

param(
    [string]$Version = "latest"
)

# Configuration
$ImageName = "synap"
$Registry = if ($env:DOCKER_REGISTRY) { $env:DOCKER_REGISTRY } else { "hivehub" }
$BuildDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

Write-Host "============================================================================" -ForegroundColor Blue
Write-Host "Synap Docker Build" -ForegroundColor Blue
Write-Host "============================================================================" -ForegroundColor Blue
Write-Host ""
Write-Host "Image:    " -NoNewline -ForegroundColor Green
Write-Host "$Registry/$ImageName"
Write-Host "Version:  " -NoNewline -ForegroundColor Green
Write-Host "$Version"
Write-Host ""

# Check if Dockerfile exists
if (-not (Test-Path "Dockerfile")) {
    Write-Host "Error: Dockerfile not found" -ForegroundColor Red
    Write-Host "Please run this script from the synap root directory"
    exit 1
}

# Enable BuildKit for cache mounts and faster builds
$env:DOCKER_BUILDKIT = "1"
$env:COMPOSE_DOCKER_CLI_BUILD = "1"

# Build the image with BuildKit optimizations
Write-Host "Building Docker image with BuildKit..." -ForegroundColor Blue
docker build `
    --progress=plain `
    -t "${Registry}/${ImageName}:${Version}" `
    -t "${Registry}/${ImageName}:latest" `
    --build-arg BUILD_DATE="$BuildDate" `
    --build-arg VERSION="$Version" `
    .

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "✓ Docker image built successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Image tags:" -ForegroundColor Blue
    Write-Host "  - ${Registry}/${ImageName}:${Version}"
    Write-Host "  - ${Registry}/${ImageName}:latest"
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Yellow
    Write-Host "  1. Test the image:    docker run -p 15500:15500 ${Registry}/${ImageName}:${Version}"
    Write-Host "  2. Push to registry:  docker push ${Registry}/${ImageName}:${Version}"
    Write-Host "  3. Deploy cluster:    docker-compose up -d"
} else {
    Write-Host ""
    Write-Host "✗ Docker build failed" -ForegroundColor Red
    exit 1
}

