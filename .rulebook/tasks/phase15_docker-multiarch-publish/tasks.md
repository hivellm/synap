## 1. Publish scripts — parity with Vectorizer build-push
- [ ] 1.1 `scripts/docker-publish.sh`: add `--sbom=true --provenance=mode=max`, registry buildx cache (`hivehub/synap-cache:buildx`, `mode=max`, `--no-cache` opt-out), builder created with explicit `--platform linux/amd64,linux/arm64`
- [ ] 1.2 `scripts/docker-publish.ps1`: same changes as 1.1 (Windows is the machine that actually publishes)

## 2. arm64 build validation
- [ ] 2.1 Build the `linux/arm64` half locally via buildx (QEMU) and confirm the aarch64 musl static binary + scratch image assemble
- [ ] 2.2 Run the arm64 image under emulation and confirm the server boots and `--health-check` exits 0

## 3. Multi-arch publish
- [ ] 3.1 Publish `hivehub/synap:1.0.0` + `latest` as a `linux/amd64,linux/arm64` manifest list with attestations
- [ ] 3.2 Verify with `docker buildx imagetools inspect` (both platforms present) and Docker Scout (0C/0H/0M/0L on both archs)

## 4. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 4.1 Update or create documentation covering the implementation (DOCKER_README.md supported-platforms section + publish instructions, CHANGELOG [1.0.0])
- [ ] 4.2 Write tests covering the new behavior (script flag parsing / dry-run smoke via `--no-push`)
- [ ] 4.3 Run tests and confirm they pass
