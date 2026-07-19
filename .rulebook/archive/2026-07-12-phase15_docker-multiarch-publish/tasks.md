## 1. Publish scripts — parity with Vectorizer build-push
- [x] 1.1 `scripts/docker-publish.sh`: add `--sbom=true --provenance=mode=max`, registry buildx cache (`hivehub/synap-cache:buildx`, `mode=max`, `--no-cache` opt-out), builder created with explicit `--platform linux/amd64,linux/arm64`
- [x] 1.2 `scripts/docker-publish.ps1`: same changes as 1.1 (Windows is the machine that actually publishes)

## 2. arm64 build validation
- [x] 2.1 Build the `linux/arm64` half locally via buildx (QEMU) and confirm the aarch64 musl static binary + scratch image assemble (16MB image, arch=arm64)
- [x] 2.2 Run the arm64 image under emulation and confirm the server boots and `--health-check` exits 0 (SIMD backend NEON runtime-detected; Docker health=healthy; direct exec EXIT=0)

## 3. Multi-arch publish
- [x] 3.1 Publish `hivehub/synap:1.0.0` + `latest` as a `linux/amd64,linux/arm64` manifest list with attestations (digest sha256:000bcd95…, SBOM+provenance manifests attached)
- [x] 3.2 Verify with `docker buildx imagetools inspect` (both platforms present) and Docker Scout (0C/0H/0M/0L on both archs)

## 4. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 4.1 Update or create documentation covering the implementation (DOCKER_README.md supported-platforms section, docs/RELEASE_PROCESS.md Docker section, CHANGELOG [1.0.0])
- [x] 4.2 Write tests covering the new behavior (scripts/test-docker-publish.sh: syntax both variants, required-flag assertions, dry-run via --no-build --no-push)
- [x] 4.3 Run tests and confirm they pass (16/16 checks green)
