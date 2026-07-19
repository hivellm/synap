# ============================================================================
# Synap Docker Publish Script (PowerShell)
# ============================================================================
#
# This script builds and publishes Synap Docker images to DockerHub
# Supports multi-arch builds (AMD64 + ARM64)
#
# Usage:
#   .\scripts\docker-publish.ps1 [version] [-NoBuild] [-NoPush] [-NoCache]
#
# Examples:
#   .\scripts\docker-publish.ps1           # Build and push latest
#   .\scripts\docker-publish.ps1 1.0.0    # Build and push specific version
#   .\scripts\docker-publish.ps1 1.0.0 -NoBuild  # Only push (skip build)
#   .\scripts\docker-publish.ps1 1.0.0 -NoPush   # Only build (skip push)
#   .\scripts\docker-publish.ps1 1.0.0 -NoCache  # Cold build (ignore registry cache)
#
# Produces a linux/amd64 + linux/arm64 manifest list with SBOM and
# provenance attestations (Docker Scout supply-chain requirements).
#
# Requirements:
#   - Docker with buildx enabled
#   - DockerHub credentials (DOCKER_USERNAME and DOCKER_PASSWORD env vars)
#
# ============================================================================

param(
    [string]$Version = "latest",
    [switch]$NoBuild = $false,
    [switch]$NoPush = $false,
    [switch]$NoCache = $false
)

# Configuration
$ImageName = "synap"
$Registry = if ($env:DOCKER_REGISTRY) { $env:DOCKER_REGISTRY } else { "hivehub" }
$DockerUsername = $env:DOCKER_USERNAME
$DockerPassword = $env:DOCKER_PASSWORD
$BuildDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
# Buildx registry cache (mode=max caches every intermediate layer, not
# just the final image). Disable with -NoCache for a cold build.
$CacheRef = if ($env:DOCKER_CACHE_REF) { $env:DOCKER_CACHE_REF } else { "hivehub/synap-cache:buildx" }

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

# Create buildx builder if it doesn't exist. The docker-container driver
# is required for multi-platform manifest lists and SBOM/provenance
# attestations (the default docker driver supports neither).
Write-Info "Setting up buildx builder..."
docker buildx create --name synap-builder --driver docker-container `
    --platform linux/amd64,linux/arm64 --use 2>&1 | Out-Null
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
    
    $buildFlags = @()
    if (-not $NoPush) {
        $buildFlags += "--push"
    }

    # Registry layer cache: always read; only write when pushing (writing
    # requires registry credentials, which a -NoPush build may not have).
    if (-not $NoCache) {
        Write-Info "Cache: ${CacheRef} (registry, mode=max)"
        $buildFlags += "--cache-from"
        $buildFlags += "type=registry,ref=${CacheRef}"
        if (-not $NoPush) {
            $buildFlags += "--cache-to"
            $buildFlags += "type=registry,ref=${CacheRef},mode=max"
        }
    } else {
        Write-Info "Cache: disabled (-NoCache)"
    }

    docker buildx build `
        --platform linux/amd64,linux/arm64 `
        $tagArgs `
        --build-arg BUILD_DATE="$BuildDate" `
        --build-arg VERSION="$Version" `
        --sbom=true `
        --provenance=mode=max `
        $buildFlags `
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
Write-Host "  2. Test:    docker run -p 15500:15500 -p 15501:15501 -p 6379:6379 ${Registry}/${ImageName}:${Version}"
Write-Host "  3. Health:  curl http://localhost:15500/health"
Write-Host "  4. View:    https://hub.docker.com/r/${Registry}/${ImageName}"

