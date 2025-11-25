# ============================================================================
# Synap Docker Publish Script (PowerShell)
# ============================================================================
#
# This script builds and publishes Synap Docker images to DockerHub
# Supports multi-arch builds (AMD64 + ARM64)
#
# Usage:
#   .\scripts\docker-publish.ps1 [version] [--no-build] [--no-push]
#
# Examples:
#   .\scripts\docker-publish.ps1           # Build and push latest
#   .\scripts\docker-publish.ps1 0.8.1    # Build and push specific version
#   .\scripts\docker-publish.ps1 0.8.1 --no-build  # Only push (skip build)
#   .\scripts\docker-publish.ps1 0.8.1 --no-push   # Only build (skip push)
#
# Requirements:
#   - Docker with buildx enabled
#   - DockerHub credentials (DOCKER_USERNAME and DOCKER_PASSWORD env vars)
#
# ============================================================================

param(
    [string]$Version = "latest",
    [switch]$NoBuild = $false,
    [switch]$NoPush = $false
)

# Configuration
$ImageName = "synap"
$Registry = if ($env:DOCKER_REGISTRY) { $env:DOCKER_REGISTRY } else { "hivehub" }
$DockerUsername = $env:DOCKER_USERNAME
$DockerPassword = $env:DOCKER_PASSWORD
$BuildDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

# Colors
function Write-Header { param([string]$Message) Write-Host "============================================================================" -ForegroundColor Blue; Write-Host $Message -ForegroundColor Blue; Write-Host "============================================================================" -ForegroundColor Blue; Write-Host "" }
function Write-Success { param([string]$Message) Write-Host "✓ $Message" -ForegroundColor Green }
function Write-Error { param([string]$Message) Write-Host "✗ $Message" -ForegroundColor Red }
function Write-Warning { param([string]$Message) Write-Host "⚠ $Message" -ForegroundColor Yellow }
function Write-Info { param([string]$Message) Write-Host "ℹ $Message" -ForegroundColor Cyan }

Write-Header "Synap Docker Publish"

Write-Info "Image:    ${Registry}/${ImageName}"
Write-Info "Version:  ${Version}"
Write-Info "Registry: DockerHub"
Write-Host ""

# Check if Dockerfile exists
if (-not (Test-Path "Dockerfile")) {
    Write-Error "Dockerfile not found"
    Write-Error "Please run this script from the synap root directory"
    exit 1
}

# Check Docker buildx
Write-Info "Checking Docker buildx..."
$buildxExists = docker buildx version 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker buildx is not available"
    Write-Error "Please install Docker with buildx support"
    exit 1
}
Write-Success "Docker buildx is available"

# Create buildx builder if it doesn't exist
Write-Info "Setting up buildx builder..."
docker buildx create --name synap-builder --use 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    # Builder might already exist, try to use it
    docker buildx use synap-builder 2>&1 | Out-Null
}
docker buildx inspect --bootstrap | Out-Null
Write-Success "Buildx builder ready"

# Login to DockerHub if pushing
if (-not $NoPush) {
    if (-not $DockerUsername -or -not $DockerPassword) {
        Write-Warning "DOCKER_USERNAME or DOCKER_PASSWORD not set"
        Write-Info "Attempting interactive login..."
        docker login
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Docker login failed"
            exit 1
        }
    } else {
        Write-Info "Logging in to DockerHub..."
        $passwordPlain = $DockerPassword
        echo $passwordPlain | docker login --username $DockerUsername --password-stdin
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Docker login failed"
            exit 1
        }
        Write-Success "Logged in to DockerHub"
    }
}

# Enable BuildKit for cache mounts and faster builds
$env:DOCKER_BUILDKIT = "1"
$env:COMPOSE_DOCKER_CLI_BUILD = "1"

# Build multi-arch image
if (-not $NoBuild) {
    Write-Info "Building multi-arch Docker image with BuildKit (linux/amd64,linux/arm64)..."
    Write-Host ""
    
    $tags = @(
        "${Registry}/${ImageName}:${Version}",
        "${Registry}/${ImageName}:latest"
    )
    
    $tagArgs = @()
    foreach ($tag in $tags) {
        $tagArgs += "-t"
        $tagArgs += $tag
    }
    
    $pushFlag = if ($NoPush) { @() } else { @("--push") }
    
    docker buildx build `
        --platform linux/amd64,linux/arm64 `
        $tagArgs `
        --build-arg BUILD_DATE="$BuildDate" `
        --build-arg VERSION="$Version" `
        $pushFlag `
        --progress=plain `
        .
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        if ($NoPush) {
            Write-Success "Docker image built successfully!"
        } else {
            Write-Success "Docker image built and pushed successfully!"
        }
        Write-Host ""
        Write-Info "Image tags:"
        foreach ($tag in $tags) {
            Write-Host "  - $tag"
        }
    } else {
        Write-Host ""
        Write-Error "Docker build failed"
        exit 1
    }
} else {
    Write-Info "Skipping build (--no-build flag)"
}

# Push to DockerHub (only if build was skipped or push was skipped during build)
if (-not $NoPush) {
    if ($NoBuild) {
        Write-Info "Pushing existing images to DockerHub..."
        
        $tags = @(
            "${Registry}/${ImageName}:${Version}",
            "${Registry}/${ImageName}:latest"
        )
        
        foreach ($tag in $tags) {
            Write-Info "Pushing $tag..."
            docker push $tag
            if ($LASTEXITCODE -eq 0) {
                Write-Success "Pushed $tag"
            } else {
                Write-Error "Failed to push $tag"
                exit 1
            }
        }
        
        Write-Host ""
        Write-Success "All images pushed successfully!"
    } else {
        Write-Info "Images already pushed during build (buildx --push)"
    }
} else {
    Write-Info "Skipping push (--no-push flag)"
}

Write-Host ""
Write-Success "Publish completed!"
Write-Host ""
Write-Info "Next steps:"
Write-Host "  1. Verify: docker pull ${Registry}/${ImageName}:${Version}"
Write-Host "  2. Test:    docker run -p 15500:15500 ${Registry}/${ImageName}:${Version}"
Write-Host "  3. View:    https://hub.docker.com/r/${Registry}/${ImageName}"

