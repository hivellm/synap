# Synap Release Process

## Overview

This document describes the automated release process for Synap using GitHub Actions.

## Release Workflow

The release workflow (`.github/workflows/release.yml`) automatically:

1. **Builds binaries** for multiple platforms:
   - Linux x64 (GNU)
   - Linux ARM64 (GNU)
   - Windows x64 (MSVC)
   - macOS x64 (Intel)
   - macOS ARM64 (Apple Silicon)

2. **Packages artifacts** for each platform:
   - `synap-server` binary
   - `synap-cli` binary
   - `README.md`, `CHANGELOG.md`, `LICENSE`
   - `config.example.yml`

3. **Creates release archives**:
   - `.tar.gz` for Unix systems (Linux, macOS)
   - `.zip` for Windows
   - SHA256 checksums for all files

4. **Publishes Docker images** to:
   - Docker Hub: `hivellm/synap:latest` and `hivellm/synap:VERSION`
   - GitHub Container Registry: `ghcr.io/hivellm/synap:latest` and `ghcr.io/hivellm/synap:VERSION`

## How to Create a Release

### Method 1: Git Tag (Recommended)

```bash
# Ensure you're on main branch and up to date
git checkout main
git pull origin main

# Update version in Cargo.toml
# Update CHANGELOG.md with release notes

# Commit changes
git add Cargo.toml CHANGELOG.md
git commit -m "chore: prepare v0.3.0 release"

# Create and push tag
git tag v0.3.0
git push origin v0.3.0

# Push commits
git push origin main
```

The workflow will automatically trigger and:
- Build binaries for all platforms
- Create GitHub Release with artifacts
- Build and push Docker images

### Method 2: Manual Workflow Dispatch

1. Go to GitHub Actions
2. Select "Release" workflow
3. Click "Run workflow"
4. Enter version (e.g., `v0.3.0`)
5. Click "Run"

## Version Naming Convention

Follow [Semantic Versioning](https://semver.org/):

- `v1.0.0` - Stable release
- `v0.3.0-rc1` - Release candidate (prerelease)
- `v0.3.0-beta1` - Beta version (prerelease)
- `v0.3.0-alpha1` - Alpha version (prerelease)

Prerelease tags (`rc`, `beta`, `alpha`) will automatically mark the GitHub Release as "Pre-release".

## Build Matrix

| Platform | Target | Output |
|----------|--------|--------|
| Linux x64 | `x86_64-unknown-linux-gnu` | `synap-linux-x64.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `synap-linux-arm64.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `synap-windows-x64.zip` |
| macOS x64 | `x86_64-apple-darwin` | `synap-macos-x64.tar.gz` |
| macOS ARM64 | `aarch64-apple-darwin` | `synap-macos-arm64.tar.gz` |

## Docker Images

Docker images are built for `linux/amd64` and `linux/arm64` platforms and pushed to:

```
hivellm/synap:latest
hivellm/synap:0.3.0

ghcr.io/hivellm/synap:latest
ghcr.io/hivellm/synap:0.3.0
```

## Release Assets

Each GitHub Release includes:

```
synap-linux-x64.tar.gz
synap-linux-x64.tar.gz.sha256
synap-linux-arm64.tar.gz
synap-linux-arm64.tar.gz.sha256
synap-windows-x64.zip
synap-windows-x64.zip.sha256
synap-macos-x64.tar.gz
synap-macos-x64.tar.gz.sha256
synap-macos-arm64.tar.gz
synap-macos-arm64.tar.gz.sha256
```

## Verifying Downloads

Users can verify downloads using SHA256 checksums:

```bash
# Linux/macOS
sha256sum -c synap-linux-x64.tar.gz.sha256

# Windows (PowerShell)
Get-FileHash synap-windows-x64.zip -Algorithm SHA256
```

## Required Secrets

The workflow requires these GitHub secrets:

- `GITHUB_TOKEN` - Automatically provided by GitHub
- `DOCKER_USERNAME` - Docker Hub username (optional, for Docker Hub push)
- `DOCKER_PASSWORD` - Docker Hub token (optional, for Docker Hub push)

## Troubleshooting

### Build Fails

- Check Rust version compatibility
- Verify all dependencies are available
- Review GitHub Actions logs

### Docker Push Fails

- Verify Docker Hub credentials in secrets
- Check Docker Hub organization permissions
- Ensure repository exists on Docker Hub

### Release Notes Empty

- Ensure CHANGELOG.md has section for the version
- Format: `## [VERSION]` (e.g., `## [0.3.0]`)
- Update CHANGELOG.md before creating tag

## Post-Release Checklist

- [ ] Verify GitHub Release created successfully
- [ ] Test download and verify checksums
- [ ] Pull Docker images and test
- [ ] Update documentation with new version
- [ ] Announce release (Twitter, Reddit, Discord)
- [ ] Create migration guide if breaking changes

## Example Release Notes

```markdown
## [0.3.0] - 2025-10-22

### Added
- Prometheus metrics endpoint (/metrics)
- Rate limiting implementation
- MCP and UMICP protocol support
- Kafka-style partitioning with consumer groups

### Changed
- Improved replication performance
- Enhanced documentation

### Fixed
- Memory leak in stream compaction
- Race condition in queue ACK

See [CHANGELOG.md](CHANGELOG.md) for complete details.
```

## Automation

The workflow is fully automated. No manual steps required except creating the git tag.

**Trigger**: Push tag matching `v*.*.*` pattern  
**Duration**: ~15-20 minutes (all platforms)  
**Output**: GitHub Release + Docker images

---

**Last Updated**: October 22, 2025  
**Workflow Version**: v1.0  
**Maintainer**: HiveLLM Team

