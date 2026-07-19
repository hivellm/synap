# Synap Release Process

## Overview

This document describes the automated release process for Synap using GitHub Actions.

## Release Workflow

The release workflow (`.github/workflows/release.yml`) follows the
Nexus/Vectorizer pipeline: it runs when a **GitHub Release is published**
(not on tag push) and uses `taiki-e/upload-rust-binary-action` to build,
archive and **attach the binaries directly to that Release**.

1. **Builds binaries** for five targets, each in an independent job (one
   target failing does not cancel the others):
   - Linux x64 (GNU) — `ubuntu-latest`
   - Linux ARM64 (GNU) — **native** `ubuntu-24.04-arm` runner (no
     cross-compilation, no vendored OpenSSL)
   - Windows x64 (MSVC) — `windows-latest`
   - macOS x64 + ARM64 — `macos-latest`

2. **Attaches to the GitHub Release**, per target:
   - `synap-server-<target>.tar.gz` (`.zip` on Windows) + `.sha256`
   - `synap-cli-<target>.tar.gz` (`.zip` on Windows) + `.sha256`

## How to Create a Release

### Method 1: Publish a GitHub Release (Recommended)

```bash
# Ensure main is up to date, version bumped in Cargo.toml and
# CHANGELOG.md has the release notes, then tag:
git tag v1.0.0
git push origin v1.0.0

# Publish the Release (this is what triggers the artifact build):
gh release create v1.0.0 --title "v1.0.0" --notes-file <(release notes)
```

Publishing the Release triggers the workflow, which attaches all binaries
and checksums to it.

### Method 2: Manual Workflow Dispatch (rebuild an existing release)

1. Go to GitHub Actions → "Release" → "Run workflow"
2. Enter the existing tag (e.g., `v1.0.0`)
3. Artifacts are (re)built from that tag and attached to its Release

## Version Naming Convention

Follow [Semantic Versioning](https://semver.org/):

- `v1.0.0` - Stable release
- `v0.3.0-rc1` - Release candidate (prerelease)
- `v0.3.0-beta1` - Beta version (prerelease)
- `v0.3.0-alpha1` - Alpha version (prerelease)

Prerelease tags (`rc`, `beta`, `alpha`) will automatically mark the GitHub Release as "Pre-release".

## Build Matrix

| Platform | Target | Assets (each + `.sha256`) |
|----------|--------|---------------------------|
| Linux x64 | `x86_64-unknown-linux-gnu` | `synap-server-x86_64-unknown-linux-gnu.tar.gz`, `synap-cli-…` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `synap-server-aarch64-unknown-linux-gnu.tar.gz`, `synap-cli-…` |
| Windows x64 | `x86_64-pc-windows-msvc` | `synap-server-x86_64-pc-windows-msvc.zip`, `synap-cli-…` |
| macOS x64 | `x86_64-apple-darwin` | `synap-server-x86_64-apple-darwin.tar.gz`, `synap-cli-…` |
| macOS ARM64 | `aarch64-apple-darwin` | `synap-server-aarch64-apple-darwin.tar.gz`, `synap-cli-…` |

## Docker Images

Docker images are published to Docker Hub as `linux/amd64` + `linux/arm64`
manifest lists with SBOM and provenance attestations:

```
hivehub/synap:latest
hivehub/synap:VERSION
```

Publishing is operator-triggered (not CI) via the publish scripts, which
handle the buildx multi-platform build, attestations and registry layer
cache (`hivehub/synap-cache:buildx`):

```bash
./scripts/docker-publish.sh 1.0.0            # bash
./scripts/docker-publish.ps1 1.0.0           # PowerShell (Windows)
```

Verify the published manifest covers both platforms:

```bash
docker buildx imagetools inspect hivehub/synap:VERSION
```

## Release Assets

Each GitHub Release includes, per binary (`synap-server`, `synap-cli`)
and per target:

```
synap-server-x86_64-unknown-linux-gnu.tar.gz        (+ .sha256)
synap-server-aarch64-unknown-linux-gnu.tar.gz       (+ .sha256)
synap-server-x86_64-apple-darwin.tar.gz             (+ .sha256)
synap-server-aarch64-apple-darwin.tar.gz            (+ .sha256)
synap-server-x86_64-pc-windows-msvc.zip             (+ .sha256)
synap-cli-<same five targets>                       (+ .sha256)
```

## Verifying Downloads

Users can verify downloads using SHA256 checksums:

```bash
# Linux/macOS
sha256sum -c synap-server-x86_64-unknown-linux-gnu.tar.gz.sha256

# Windows (PowerShell)
Get-FileHash synap-server-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

## Required Secrets

- `GITHUB_TOKEN` - Automatically provided by GitHub (attaches assets to
  the Release)
- Docker Hub credentials are NOT needed here — Docker publishing is
  operator-triggered via `scripts/docker-publish.{sh,ps1}` (see the
  Docker Images section above)

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

