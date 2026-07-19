# Spec: Multi-arch Docker publish

## ADDED Requirements

### Requirement: Multi-arch manifest list on Docker Hub
The published `hivehub/synap` image MUST be a manifest list covering
`linux/amd64` and `linux/arm64`, for both the version tag and `latest`.

#### Scenario: arm64 host pulls natively
Given an arm64 host (Apple Silicon / Graviton)
When it runs `docker pull hivehub/synap:1.0.0`
Then Docker resolves the `linux/arm64` manifest and runs the native binary
without emulation

#### Scenario: manifest inspection
Given the image was published by the publish script
When `docker buildx imagetools inspect hivehub/synap:1.0.0` runs
Then the output lists both `linux/amd64` and `linux/arm64` platforms

### Requirement: Supply-chain attestations
The publish scripts SHALL build with `--sbom=true` and
`--provenance=mode=max` so every published manifest carries SBOM and
provenance attestations.

#### Scenario: Docker Scout attestation finding stays closed
Given the multi-arch image was published with attestations
When Docker Scout analyzes `hivehub/synap:1.0.0`
Then no "Missing supply chain attestation(s)" finding is reported for
either architecture

### Requirement: Registry buildx cache
The publish scripts SHALL read and write a registry buildx cache
(`hivehub/synap-cache:buildx`, `mode=max`) and MUST offer an opt-out flag
for cold builds.

#### Scenario: warm rebuild reuses layers
Given a prior publish populated the cache repository
When the publish script runs again with an unchanged Dockerfile prefix
Then buildx restores intermediate layers from the registry cache instead
of recompiling them

### Requirement: arm64 image is functional
The `linux/arm64` image MUST boot the server and pass its built-in health
probe, verified at least once before a publish that includes arm64.

#### Scenario: health probe under emulation
Given the arm64 image built via QEMU on an x86_64 host
When the container starts and `synap-server --health-check` executes
Then the probe exits with status 0
