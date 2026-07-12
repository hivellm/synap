# Spec: Release artifacts CI

## ADDED Requirements

### Requirement: Binaries attached to the GitHub Release
Publishing a GitHub Release MUST result in `synap-server` and `synap-cli`
archives, each with a SHA-256 checksum asset, attached to that Release for
Linux x64/arm64, macOS x64/arm64 and Windows x64.

#### Scenario: release publication ships binaries
Given a GitHub Release is published for tag `vX.Y.Z`
When the Release workflow completes
Then the Release page lists `synap-server-<target>` and
`synap-cli-<target>` archives with `.sha256` companions for all five
targets

### Requirement: Manual re-run for an existing tag
The workflow SHALL support `workflow_dispatch` with a `tag` input so an
operator can (re)build artifacts for an already-published release.

#### Scenario: dispatch on existing tag
Given release `vX.Y.Z` exists without artifacts
When the workflow is dispatched with `tag: vX.Y.Z`
Then the workflow checks out that tag and attaches the artifacts to that
release

### Requirement: Native arm64 build
The `aarch64-unknown-linux-gnu` artifact MUST be produced on a native
arm64 runner without cross-compilation toolchains or vendored OpenSSL.

#### Scenario: arm64 job runs natively
Given the workflow runs
When the linux-arm64 job executes
Then it runs on `ubuntu-24.04-arm` and builds without
`--features vendored-openssl` and without installing a cross gcc

### Requirement: Independent target jobs
A failure in one target's job MUST NOT cancel the other targets' jobs.

#### Scenario: one target fails
Given the macOS job fails
When the remaining jobs continue
Then Linux and Windows artifacts are still attached to the release
