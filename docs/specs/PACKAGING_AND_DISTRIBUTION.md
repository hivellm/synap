# Packaging and Distribution

## Overview

Synap provides native installers and packages for all major operating systems, ensuring easy installation and updates across different platforms.

## Supported Platforms

| Platform | Package Format | Installation Method | Auto-Update |
|----------|---------------|---------------------|-------------|
| Windows | MSI | Windows Installer | ✅ |
| Linux (Debian/Ubuntu) | DEB | apt-get / dpkg | ✅ |
| Linux (RHEL/Fedora) | RPM | yum / dnf | ✅ |
| macOS | Homebrew | brew | ✅ |
| Docker | Container | docker pull | ✅ |
| Source | Cargo | cargo install | Manual |

---

## Windows (MSI Installer)

### Building MSI Package

Synap uses [cargo-wix](https://github.com/volks73/cargo-wix) to generate professional Windows installers.

#### Prerequisites

```powershell
# Install WiX Toolset
winget install --id=WixToolset.WixToolset -e

# Install cargo-wix
cargo install cargo-wix

# Verify installation
wix --version
```

#### Build Configuration

**`wix/main.wxs`** - WiX XML Configuration:
```xml
<?xml version='1.0' encoding='windows-1252'?>
<Wix xmlns='http://schemas.microsoft.com/wix/2006/wi'>
  <Product
    Id='*'
    Name='Synap'
    Language='1033'
    Version='0.1.0'
    Manufacturer='HiveLLM'
    UpgradeCode='12345678-1234-1234-1234-123456789012'>
    
    <Package
      InstallerVersion='200'
      Compressed='yes'
      InstallScope='perMachine'
      Comments='High-Performance Data Infrastructure'
      Description='Synap - In-Memory Key-Value Store and Message Broker'
    />

    <Media Id='1' Cabinet='synap.cab' EmbedCab='yes' />

    <Directory Id='TARGETDIR' Name='SourceDir'>
      <Directory Id='ProgramFiles64Folder'>
        <Directory Id='INSTALLDIR' Name='Synap'>
          
          <!-- Binaries -->
          <Component Id='MainBinary' Guid='*'>
            <File
              Id='SynapServerExe'
              Name='synap-server.exe'
              DiskId='1'
              Source='target\release\synap-server.exe'
              KeyPath='yes'
            />
            
            <!-- Environment Variable -->
            <Environment
              Id='PATH'
              Name='PATH'
              Value='[INSTALLDIR]'
              Permanent='no'
              Part='last'
              Action='set'
              System='yes'
            />
          </Component>

          <!-- Configuration Files -->
          <Component Id='ConfigFiles' Guid='*'>
            <File
              Id='DefaultConfig'
              Name='config.yml'
              Source='config.example.yml'
            />
          </Component>

          <!-- Service Installation -->
          <Component Id='ServiceComponent' Guid='*'>
            <File
              Id='ServiceExe'
              Name='synap-service.exe'
              Source='target\release\synap-service.exe'
            />
            
            <ServiceInstall
              Id='SynapService'
              Name='Synap'
              DisplayName='Synap Data Infrastructure'
              Description='High-performance in-memory key-value store and message broker'
              Type='ownProcess'
              Start='auto'
              ErrorControl='normal'
              Arguments='--config "[INSTALLDIR]config.yml"'
            />
            
            <ServiceControl
              Id='StartService'
              Start='install'
              Stop='both'
              Remove='uninstall'
              Name='Synap'
              Wait='yes'
            />
          </Component>

        </Directory>
      </Directory>

      <!-- Start Menu Shortcuts -->
      <Directory Id='ProgramMenuFolder'>
        <Directory Id='ApplicationProgramsFolder' Name='Synap'>
          <Component Id='ApplicationShortcut' Guid='*'>
            <Shortcut
              Id='SynapStartMenuShortcut'
              Name='Synap Server'
              Description='Start Synap Server'
              Target='[INSTALLDIR]synap-server.exe'
              WorkingDirectory='INSTALLDIR'
            />
            <RemoveFolder Id='ApplicationProgramsFolder' On='uninstall'/>
            <RegistryValue
              Root='HKCU'
              Key='Software\HiveLLM\Synap'
              Name='installed'
              Type='integer'
              Value='1'
              KeyPath='yes'
            />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <!-- Features -->
    <Feature Id='Complete' Level='1'>
      <ComponentRef Id='MainBinary' />
      <ComponentRef Id='ConfigFiles' />
      <ComponentRef Id='ServiceComponent' />
      <ComponentRef Id='ApplicationShortcut' />
    </Feature>

    <!-- UI -->
    <UIRef Id='WixUI_Minimal' />
    <WixVariable Id='WixUILicenseRtf' Value='LICENSE.rtf' />
    
  </Product>
</Wix>
```

#### Build Commands

```powershell
# Build release binary
cargo build --release

# Generate WiX source (first time)
cargo wix init

# Build MSI installer
cargo wix --nocapture

# Output: target/wix/synap-0.1.0-x86_64.msi
```

#### Advanced Build Options

```powershell
# Custom version
cargo wix --version 1.0.0

# With specific features
cargo wix --features "mcp,umicp,persistence"

# Sign the MSI (code signing certificate required)
signtool sign /f cert.pfx /p password /t http://timestamp.digicert.com target/wix/synap-0.1.0-x86_64.msi
```

### Installation

**End-User Installation**:
```powershell
# GUI Installation
.\synap-0.1.0-x86_64.msi

# Silent Installation
msiexec /i synap-0.1.0-x86_64.msi /quiet /norestart

# Install to custom directory
msiexec /i synap-0.1.0-x86_64.msi INSTALLDIR="C:\Custom\Path" /quiet

# Install with logging
msiexec /i synap-0.1.0-x86_64.msi /l*v install.log
```

### Service Management

```powershell
# Start service
sc start Synap

# Stop service
sc stop Synap

# Check status
sc query Synap

# Configure startup
sc config Synap start= auto
```

### Uninstallation

```powershell
# Via Control Panel
# Settings > Apps > Synap > Uninstall

# Silent uninstall
msiexec /x synap-0.1.0-x86_64.msi /quiet
```

---

## Linux (Debian/Ubuntu - DEB Package)

### Building DEB Package

Synap uses [cargo-deb](https://github.com/kornelski/cargo-deb) for Debian package generation.

#### Prerequisites

```bash
# Install cargo-deb
cargo install cargo-deb

# Install build dependencies
sudo apt-get install -y build-essential dpkg-dev
```

#### Build Configuration

**`Cargo.toml`** - Add debian metadata:
```toml
[package.metadata.deb]
maintainer = "HiveLLM Team <contact@hivellm.org>"
copyright = "2025, HiveLLM <contact@hivellm.org>"
license-file = ["LICENSE", "0"]
extended-description = """\
Synap is a high-performance in-memory key-value store and message broker
built in Rust, combining features from Redis, RabbitMQ, and Kafka."""
depends = "$auto, systemd"
section = "database"
priority = "optional"
assets = [
    ["target/release/synap-server", "usr/bin/", "755"],
    ["config.example.yml", "etc/synap/config.yml", "644"],
    ["README.md", "usr/share/doc/synap/README", "644"],
    ["docs/**/*", "usr/share/doc/synap/", "644"],
]

# Systemd service
[package.metadata.deb.systemd-units]
unit-scripts = "debian/synap.service"
enable = true
```

**`debian/synap.service`** - Systemd Service:
```ini
[Unit]
Description=Synap Data Infrastructure
Documentation=https://github.com/hivellm/synap
After=network.target

[Service]
Type=simple
User=synap
Group=synap
ExecStart=/usr/bin/synap-server --config /etc/synap/config.yml
Restart=on-failure
RestartSec=5s

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/synap /var/log/synap

# Resource Limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

**`debian/postinst`** - Post-installation script:
```bash
#!/bin/bash
set -e

# Create synap user
if ! id -u synap >/dev/null 2>&1; then
    useradd --system --home-dir /var/lib/synap --shell /bin/false synap
fi

# Create directories
mkdir -p /var/lib/synap
mkdir -p /var/log/synap
chown -R synap:synap /var/lib/synap
chown -R synap:synap /var/log/synap

# Reload systemd
systemctl daemon-reload

# Enable and start service
systemctl enable synap.service
systemctl start synap.service

echo "Synap installed successfully!"
echo "Service status: systemctl status synap"
```

#### Build Commands

```bash
# Build DEB package
cargo deb

# Output: target/debian/synap_0.1.0_amd64.deb

# Build for specific architecture
cargo deb --target=aarch64-unknown-linux-gnu

# With specific features
cargo deb --features "mcp,umicp,persistence"
```

### Repository Setup (APT)

**Host packages on APT repository**:

```bash
# Install aptly (repository management tool)
sudo apt-get install aptly

# Create repository
aptly repo create -distribution=stable -component=main synap-repo

# Add package
aptly repo add synap-repo target/debian/synap_0.1.0_amd64.deb

# Publish repository
aptly publish repo -architectures=amd64 synap-repo

# Serve repository (for testing)
aptly serve

# Or sync to S3/web server
aws s3 sync ~/.aptly/public/ s3://packages.synap.io/apt/
```

**`/etc/apt/sources.list.d/synap.list`**:
```bash
deb [trusted=yes] https://packages.synap.io/apt stable main
```

### Installation

```bash
# Add repository
curl -fsSL https://packages.synap.io/gpg.key | sudo apt-key add -
echo "deb [arch=amd64] https://packages.synap.io/apt stable main" | sudo tee /etc/apt/sources.list.d/synap.list

# Update and install
sudo apt-get update
sudo apt-get install synap

# Check service status
systemctl status synap

# View logs
journalctl -u synap -f
```

### Configuration

```bash
# Edit configuration
sudo nano /etc/synap/config.yml

# Restart service
sudo systemctl restart synap

# Enable on boot
sudo systemctl enable synap
```

### Uninstallation

```bash
# Stop service
sudo systemctl stop synap

# Remove package
sudo apt-get remove synap

# Purge (remove config)
sudo apt-get purge synap

# Remove repository
sudo rm /etc/apt/sources.list.d/synap.list
```

---

## Linux (RHEL/Fedora - RPM Package)

### Building RPM Package

```bash
# Install cargo-rpm
cargo install cargo-rpm

# Build RPM
cargo rpm build

# Output: target/release/rpmbuild/RPMS/x86_64/synap-0.1.0-1.x86_64.rpm
```

### Installation

```bash
# Install
sudo rpm -i synap-0.1.0-1.x86_64.rpm

# Or via DNF
sudo dnf install ./synap-0.1.0-1.x86_64.rpm

# Uninstall
sudo rpm -e synap
```

---

## macOS (Homebrew)

### Homebrew Formula

**`Formula/synap.rb`** - Homebrew Formula:
```ruby
class Synap < Formula
  desc "High-performance in-memory key-value store and message broker"
  homepage "https://github.com/hivellm/synap"
  url "https://github.com/hivellm/synap/archive/v0.1.0.tar.gz"
  sha256 "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
  license "MIT"
  head "https://github.com/hivellm/synap.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
    
    # Install config
    etc.install "config.example.yml" => "synap/config.yml"
    
    # Create data directory
    (var/"lib/synap").mkpath
    (var/"log/synap").mkpath
  end

  service do
    run [opt_bin/"synap-server", "--config", etc/"synap/config.yml"]
    keep_alive true
    log_path var/"log/synap/synap.log"
    error_log_path var/"log/synap/synap-error.log"
  end

  test do
    system "#{bin}/synap-server", "--version"
  end
end
```

### Tap Repository

Create a Homebrew tap repository:

```bash
# Create tap repository
gh repo create hivellm/homebrew-synap --public

# Clone and add formula
git clone https://github.com/hivellm/homebrew-synap
cd homebrew-synap
mkdir Formula
cp synap.rb Formula/

git add .
git commit -m "Add Synap formula"
git push
```

### Installation

```bash
# Add tap
brew tap hivellm/synap

# Install
brew install synap

# Start service
brew services start synap

# Stop service
brew services stop synap

# Restart service
brew services restart synap

# Check status
brew services info synap
```

### Configuration

```bash
# Edit config
nano /usr/local/etc/synap/config.yml

# Or via Homebrew
brew edit synap

# Restart to apply changes
brew services restart synap
```

### Uninstallation

```bash
# Stop service
brew services stop synap

# Uninstall
brew uninstall synap

# Remove tap
brew untap hivellm/synap
```

---

## Docker Image

### Dockerfile

**`Dockerfile`**:
```dockerfile
# Build stage
FROM rust:1.82-slim as builder

WORKDIR /build

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY . .

# Build release
RUN cargo build --release --features full

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create synap user
RUN useradd --system --create-home --home-dir /var/lib/synap synap

# Copy binary
COPY --from=builder /build/target/release/synap-server /usr/local/bin/
COPY config.example.yml /etc/synap/config.yml

# Create directories
RUN mkdir -p /var/lib/synap /var/log/synap && \
    chown -R synap:synap /var/lib/synap /var/log/synap /etc/synap

# Switch to synap user
USER synap

# Expose ports
EXPOSE 15500 15501

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:15500/health || exit 1

# Start server
CMD ["synap-server", "--config", "/etc/synap/config.yml"]
```

### Build and Publish

```bash
# Build image
docker build -t synap:latest .
docker build -t synap:0.1.0 .

# Tag for registry
docker tag synap:latest hivellm/synap:latest
docker tag synap:0.1.0 hivellm/synap:0.1.0

# Push to Docker Hub
docker push hivellm/synap:latest
docker push hivellm/synap:0.1.0

# Push to GitHub Container Registry
docker tag synap:latest ghcr.io/hivellm/synap:latest
docker push ghcr.io/hivellm/synap:latest
```

### Usage

```bash
# Run container
docker run -d \
  --name synap \
  -p 15500:15500 \
  -p 15501:15501 \
  -v synap-data:/var/lib/synap \
  -v synap-logs:/var/log/synap \
  hivellm/synap:latest

# With custom config
docker run -d \
  --name synap \
  -p 15500:15500 \
  -v $(pwd)/config.yml:/etc/synap/config.yml \
  hivellm/synap:latest

# Docker Compose
docker-compose up -d
```

**`docker-compose.yml`**:
```yaml
version: '3.8'

services:
  synap-master:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
      - "15501:15501"
    volumes:
      - synap-master-data:/var/lib/synap
      - synap-master-logs:/var/log/synap
      - ./config-master.yml:/etc/synap/config.yml
    environment:
      - SYNAP_MODE=master
    restart: unless-stopped

  synap-replica:
    image: hivellm/synap:latest
    ports:
      - "15502:15500"
    volumes:
      - synap-replica-data:/var/lib/synap
      - synap-replica-logs:/var/log/synap
      - ./config-replica.yml:/etc/synap/config.yml
    environment:
      - SYNAP_MODE=replica
      - SYNAP_MASTER_HOST=synap-master
    depends_on:
      - synap-master
    restart: unless-stopped

volumes:
  synap-master-data:
  synap-master-logs:
  synap-replica-data:
  synap-replica-logs:
```

---

## Continuous Integration / Release Pipeline

### GitHub Actions Workflow

**`.github/workflows/release.yml`**:
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal
      
      - name: Install WiX
        run: dotnet tool install --global wix
      
      - name: Build MSI
        run: |
          cargo build --release --features full
          cargo wix --nocapture
      
      - name: Upload MSI
        uses: actions/upload-artifact@v3
        with:
          name: synap-windows-msi
          path: target/wix/*.msi

  build-linux-deb:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal
      
      - name: Install cargo-deb
        run: cargo install cargo-deb
      
      - name: Build DEB
        run: cargo deb --features full
      
      - name: Upload DEB
        uses: actions/upload-artifact@v3
        with:
          name: synap-linux-deb
          path: target/debian/*.deb

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal
      
      - name: Build
        run: cargo build --release --features full
      
      - name: Create tarball
        run: |
          tar czf synap-macos-x86_64.tar.gz \
            -C target/release synap-server
      
      - name: Upload tarball
        uses: actions/upload-artifact@v3
        with:
          name: synap-macos-tarball
          path: synap-macos-x86_64.tar.gz

  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2
      
      - name: Login to Docker Hub
        uses: docker/login-action@v2
        with:
          username: ${{ secrets.DOCKER_USERNAME }}
          password: ${{ secrets.DOCKER_PASSWORD }}
      
      - name: Build and push
        uses: docker/build-push-action@v4
        with:
          context: .
          push: true
          tags: |
            hivellm/synap:latest
            hivellm/synap:${{ github.ref_name }}
          platforms: linux/amd64,linux/arm64

  release:
    needs: [build-windows, build-linux-deb, build-macos, docker]
    runs-on: ubuntu-latest
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v3
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            synap-windows-msi/*.msi
            synap-linux-deb/*.deb
            synap-macos-tarball/*.tar.gz
          draft: false
          prerelease: false
```

---

## Version Management

### Versioning Strategy

Synap follows [Semantic Versioning](https://semver.org/):

```
MAJOR.MINOR.PATCH
  │     │     │
  │     │     └─ Bug fixes
  │     └─ New features (backwards compatible)
  └─ Breaking changes
```

### Update Channels

- **Stable**: Production-ready releases
- **Beta**: Pre-release testing
- **Nightly**: Daily builds from main branch

---

## Distribution Checklist

Before releasing a new version:

- [ ] Update version in `Cargo.toml`
- [ ] Update CHANGELOG.md
- [ ] Run full test suite
- [ ] Build and test all packages (MSI, DEB, Homebrew)
- [ ] Test installation on clean systems
- [ ] Verify service management works
- [ ] Update documentation
- [ ] Tag release in Git
- [ ] Trigger CI/CD pipeline
- [ ] Publish Docker images
- [ ] Update package repositories
- [ ] Announce release

---

## See Also

- [Deployment Guide](DEPLOYMENT.md)
- [Configuration Reference](CONFIGURATION.md)
- [Development Guide](DEVELOPMENT.md)
- [CI/CD Documentation](../devops/CI_CD.md)

