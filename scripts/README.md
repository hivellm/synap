# Build Scripts

Automated build scripts for creating platform-specific installers and packages.

## Available Scripts

### `build-windows.ps1`

Build Windows MSI installer using WiX Toolset.

**Requirements**:
- PowerShell 5.1+
- Rust toolchain
- WiX Toolset (auto-installed)
- cargo-wix (auto-installed)

**Usage**:
```powershell
# Build with default version
.\scripts\build-windows.ps1

# Build with specific version
.\scripts\build-windows.ps1 -Version "1.0.0"
```

**Output**: `target/wix/synap-{version}-x86_64.msi`

---

### `build-linux.sh`

Build Debian/Ubuntu DEB package.

**Requirements**:
- Bash
- Rust toolchain
- cargo-deb (auto-installed)
- build-essential, dpkg-dev

**Usage**:
```bash
# Build with default version
./scripts/build-linux.sh

# Build with specific version
./scripts/build-linux.sh 1.0.0

# Build and test installation
./scripts/build-linux.sh 1.0.0 --test
```

**Output**: `target/debian/synap_{version}_amd64.deb`

---

### `build-macos.sh`

Build macOS tarball and Homebrew formula.

**Requirements**:
- Bash
- Rust toolchain
- tar, shasum

**Usage**:
```bash
# Build with default version
./scripts/build-macos.sh

# Build with specific version
./scripts/build-macos.sh 1.0.0

# Build and test Homebrew installation
./scripts/build-macos.sh 1.0.0 --test
```

**Output**: 
- `synap-macos-{arch}-{version}.tar.gz`
- `Formula/synap.rb`

---

### `build-all.sh`

Build packages for all platforms (detects current OS).

**Usage**:
```bash
# Build for current platform
./scripts/build-all.sh

# Build with specific version
./scripts/build-all.sh 1.0.0
```

---

## Build Process

### 1. Windows (MSI)

```powershell
# Prerequisites check
- Verify Rust installation
- Install WiX Toolset if needed
- Install cargo-wix if needed

# Build
- cargo build --release --features full
- cargo wix --nocapture --version {version}

# Output
- MSI installer in target/wix/
- Includes service installation
- Adds to PATH automatically
```

### 2. Linux (DEB)

```bash
# Prerequisites check
- Verify Rust installation
- Install cargo-deb if needed
- Check build-essential

# Build
- cargo build --release --features full
- cargo deb --no-build

# Output
- DEB package in target/debian/
- Includes systemd service
- Creates synap user
- Post-install script runs automatically
```

### 3. macOS (Homebrew)

```bash
# Prerequisites check
- Verify Rust installation

# Build
- cargo build --release --features full
- Create distribution tarball
- Generate SHA256 checksum
- Create Homebrew formula

# Output
- Tarball for GitHub releases
- Formula for Homebrew tap
```

---

## CI/CD Integration

These scripts are used in GitHub Actions workflow:

```yaml
# .github/workflows/release.yml
- name: Build Windows MSI
  run: pwsh scripts/build-windows.ps1 ${{ github.ref_name }}

- name: Build Linux DEB
  run: ./scripts/build-linux.sh ${{ github.ref_name }}

- name: Build macOS Package
  run: ./scripts/build-macos.sh ${{ github.ref_name }}
```

---

## Testing Packages

### Windows
```powershell
# Install
msiexec /i synap-0.1.0-x86_64.msi /l*v install.log

# Verify
synap-server --version
sc query Synap

# Uninstall
msiexec /x synap-0.1.0-x86_64.msi /quiet
```

### Linux
```bash
# Install
sudo dpkg -i synap_0.1.0_amd64.deb

# Verify
synap-server --version
systemctl status synap

# Uninstall
sudo apt-get remove synap
```

### macOS
```bash
# Install from formula
brew install --build-from-source Formula/synap.rb

# Verify
synap-server --version
brew services info synap

# Uninstall
brew uninstall synap
```

---

## Troubleshooting

### Windows: WiX Installation Failed
```powershell
# Manual installation
winget install --id=WixToolset.WixToolset -e
```

### Linux: cargo-deb Not Found
```bash
# Install manually
cargo install cargo-deb
```

### macOS: Permission Denied
```bash
# Make script executable
chmod +x scripts/build-macos.sh
```

### All Platforms: Rust Not Found
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

---

## Version Management

Update version in multiple places:

1. **Cargo.toml**: `version = "0.1.0"`
2. **WiX main.wxs**: `<Product Version='0.1.0' .../>`
3. **Pass as parameter**: `./build-linux.sh 0.1.0`

Or use the version parameter in build scripts to override.

---

---

## Docker Scripts

### `docker-build.ps1` / `docker-build.sh`

Build Docker images locally for testing.

**Usage**:
```powershell
# PowerShell
.\scripts\docker-build.ps1 [version]

# Bash
./scripts/docker-build.sh [version]
```

**Examples**:
```powershell
# Build latest
.\scripts\docker-build.ps1

# Build specific version
.\scripts\docker-build.ps1 0.8.1
```

**Output**: Local Docker image tagged as `hivehub/synap:{version}` and `hivehub/synap:latest`

---

### `docker-publish.ps1` / `docker-publish.sh`

Build and publish multi-arch Docker images to DockerHub.

**Requirements**:
- Docker with buildx enabled
- DockerHub credentials (DOCKER_USERNAME and DOCKER_PASSWORD env vars)

**Usage**:
```powershell
# PowerShell
.\scripts\docker-publish.ps1 [version] [--no-build] [--no-push]

# Bash
./scripts/docker-publish.sh [version] [--no-build] [--no-push]
```

**Examples**:
```powershell
# Build and push latest
.\scripts\docker-publish.ps1

# Build and push specific version
.\scripts\docker-publish.ps1 0.8.1

# Only push (skip build)
.\scripts\docker-publish.ps1 0.8.1 --no-build

# Only build (skip push)
.\scripts\docker-publish.ps1 0.8.1 --no-push
```

**Features**:
- Multi-arch support (AMD64 + ARM64)
- Automatic DockerHub login
- Buildx builder setup
- Progress output

**Output**: Published to DockerHub as `hivehub/synap:{version}` and `hivehub/synap:latest`

---

### `docker-deploy.ps1` / `docker-deploy.sh`

Manage Synap replication cluster using Docker Compose.

**Usage**:
```powershell
# PowerShell
.\scripts\docker-deploy.ps1 [command]

# Bash
./scripts/docker-deploy.sh [command]
```

**Commands**:
- `start` - Start the cluster (1 master + 3 replicas)
- `stop` - Stop the cluster
- `restart` - Restart the cluster
- `status` - Show cluster status
- `logs` - Show logs (all nodes)
- `health` - Check health of all nodes
- `clean` - Stop and remove all data (DANGER!)

**Examples**:
```powershell
.\scripts\docker-deploy.ps1 start
.\scripts\docker-deploy.ps1 logs
.\scripts\docker-deploy.ps1 health
```

---

## See Also

- [Packaging & Distribution Guide](../docs/PACKAGING_AND_DISTRIBUTION.md)
- [Deployment Guide](../docs/DEPLOYMENT.md)
- [Development Guide](../docs/DEVELOPMENT.md)

