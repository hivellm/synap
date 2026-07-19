# Build Windows MSI Installer
# Usage: .\scripts\build-windows.ps1 [version]

param(
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

Write-Host "Building Synap MSI Installer v$Version" -ForegroundColor Green

# Check prerequisites
Write-Host "`nChecking prerequisites..." -ForegroundColor Yellow

# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Rust/Cargo not found. Install from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Check WiX
if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    Write-Host "Installing WiX Toolset..." -ForegroundColor Yellow
    winget install --id=WixToolset.WixToolset -e --accept-source-agreements --accept-package-agreements
}

# Check cargo-wix
if (-not (Get-Command cargo-wix -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-wix..." -ForegroundColor Yellow
    cargo install cargo-wix
}

# Clean previous builds
Write-Host "`nCleaning previous builds..." -ForegroundColor Yellow
if (Test-Path "target\release") {
    Remove-Item -Recurse -Force "target\release\synap-server.exe" -ErrorAction SilentlyContinue
}
if (Test-Path "target\wix") {
    Remove-Item -Recurse -Force "target\wix" -ErrorAction SilentlyContinue
}

# Build release binary
Write-Host "`nBuilding release binary..." -ForegroundColor Yellow
cargo build --release --features full

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Build failed" -ForegroundColor Red
    exit 1
}

# Build MSI
Write-Host "`nBuilding MSI installer..." -ForegroundColor Yellow
cargo wix --nocapture --version $Version

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: MSI build failed" -ForegroundColor Red
    exit 1
}

# Find generated MSI
$MsiPath = Get-ChildItem -Path "target\wix" -Filter "*.msi" | Select-Object -First 1

if ($MsiPath) {
    Write-Host "`nSuccess! MSI installer created:" -ForegroundColor Green
    Write-Host "  $($MsiPath.FullName)" -ForegroundColor Cyan
    Write-Host "`nFile size: $([math]::Round($MsiPath.Length / 1MB, 2)) MB" -ForegroundColor Gray
    
    # Optional: Sign the MSI
    if ($env:CODE_SIGN_CERT) {
        Write-Host "`nSigning MSI..." -ForegroundColor Yellow
        signtool sign /f $env:CODE_SIGN_CERT /p $env:CODE_SIGN_PASSWORD /t http://timestamp.digicert.com $MsiPath.FullName
    }
    
    Write-Host "`nInstallation:" -ForegroundColor Yellow
    Write-Host "  GUI:    $($MsiPath.Name)" -ForegroundColor Gray
    Write-Host "  Silent: msiexec /i $($MsiPath.Name) /quiet" -ForegroundColor Gray
} else {
    Write-Host "Error: MSI file not found" -ForegroundColor Red
    exit 1
}

