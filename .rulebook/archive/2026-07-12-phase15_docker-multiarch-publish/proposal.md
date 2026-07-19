# Proposal: Multi-arch Docker publish (amd64 + arm64) to Docker Hub

## Why

The `hivehub/synap:1.0.0` image currently on Docker Hub was built and pushed
single-arch (`linux/amd64` only, plain `docker build` + `docker push`).
Apple Silicon Macs, AWS Graviton, Raspberry Pi and other arm64 hosts either
fail to pull or run the amd64 image under slow emulation. The sibling
project **Vectorizer** already publishes `linux/amd64,linux/arm64` manifest
lists with SBOM + provenance attestations via
`scripts/docker/build-push.ps1` (buildx docker-container driver, registry
layer cache) — Synap must reach the same publishing standard.

What already exists in Synap:

- `Dockerfile` handles `TARGETARCH` correctly (amd64/arm64 →
  `x86_64/aarch64-unknown-linux-musl` triples, fully static binary,
  `FROM scratch` runtime) — the arm64 *build path* is designed but has
  never been exercised end to end.
- `scripts/docker-publish.sh` and `scripts/docker-publish.ps1` already pass
  `--platform linux/amd64,linux/arm64` — but neither emits SBOM/provenance
  attestations, neither uses a registry buildx cache, and neither has been
  used for a real multi-arch push.

## What Changes

1. **`scripts/docker-publish.sh`** — add `--sbom=true --provenance=mode=max`
   (closes the "Missing supply chain attestation(s)" Docker Scout finding on
   the arm64 half), add registry buildx cache
   (`hivehub/synap-cache:buildx`, `mode=max`) with a `--no-cache` opt-out,
   create the builder with an explicit
   `--platform linux/amd64,linux/arm64` list, and pass `VERSION`/`BUILD_DATE`
   build args (parity with Vectorizer's `build-push.ps1`).
2. **`scripts/docker-publish.ps1`** — same changes as the `.sh` variant so
   the Windows dev machine (the one that actually publishes today) produces
   identical artifacts.
3. **arm64 build validation** — run the aarch64 half of the build (QEMU
   emulation via the buildx container driver) and confirm the
   `aarch64-unknown-linux-musl` static binary compiles, the scratch image
   assembles, and `--health-check` works on the arm64 image.
4. **Publish `1.0.0` + `latest` as a manifest list** — one
   `docker buildx build --platform linux/amd64,linux/arm64 --push`
   producing a manifest list covering both archs, verified with
   `docker buildx imagetools inspect` and Docker Scout (must stay
   0C/0H/0M/0L on both).
5. **Docs** — DOCKER_README.md and docs/ get the multi-arch statement
   (supported platforms table, publish instructions).

## Reference

Vectorizer: `e:/HiveLLM/Vectorizer/scripts/docker/build-push.ps1`
(builder `vectorizer-builder`, docker-container driver, platforms
`linux/amd64,linux/arm64`, `--provenance mode=max --sbom true`,
registry cache `hivehub/vectorizer-cache:buildx` with `mode=max`).

## Impact

- Affected specs: `specs/docker-publish/spec.md` (new)
- Affected code: `scripts/docker-publish.sh`, `scripts/docker-publish.ps1`,
  `DOCKER_README.md`, `docs/deployment/*` (Dockerfile itself should need no
  change — arm64 support is already designed in)
- Breaking change: NO (same tags, superset of platforms)
- User benefit: `docker pull hivehub/synap` works natively on Apple
  Silicon / Graviton / arm64; supply-chain attestations on every published
  manifest

## Out of scope

- CI-driven publishing (GitHub Actions) — publishing stays a local,
  operator-triggered script run, same as Vectorizer.
- Windows/macOS container images.
