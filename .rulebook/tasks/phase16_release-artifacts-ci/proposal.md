# Proposal: Release artifacts CI following Nexus/Vectorizer

## Why

Synap's `release.yml` builds tarballs but uploads them with
`actions/upload-artifact` — they live inside the workflow run, expire, and
are **never attached to the GitHub Release**. Users landing on a release
page find no binaries. The sibling projects solved this:

- **Nexus** (`release-server.yml`/`release-cli.yml`): builds with
  `taiki-e/upload-rust-binary-action@v1`, which compiles, archives and
  **attaches the artifact directly to the GitHub Release**; triggers on
  `release: published` plus a `workflow_dispatch` tag input resolved once
  into a `RELEASE_TAG` env.
- **Vectorizer** (`release-artifacts.yml`): same action, plus **native
  ARM64 runners** (`ubuntu-24.04-arm`) instead of cross/QEMU,
  `CARGO_INCREMENTAL: "0"` (cold CI builds), and independent per-target
  jobs so one target's failure doesn't cancel the others.

Synap's aarch64-linux release build also currently needs the
cross-compiler + `vendored-openssl` workaround; a native ARM runner
(free for public repos — hivellm/synap is public) removes it.

## What Changes

Rewrite `.github/workflows/release.yml` on the reference pattern:

1. **Trigger**: `release: types [published]` + `workflow_dispatch` with a
   `tag` input (Nexus `RELEASE_TAG` env pattern). The current tag-push
   trigger goes away — creating the GitHub Release is what publishes
   binaries.
2. **Delivery**: `taiki-e/upload-rust-binary-action@v1` per binary per
   target, with `checksum: sha256` (RELEASE_PROCESS.md already documents
   `.sha256` assets). Both `synap-server` and `synap-cli` ship.
3. **Targets** (same five as today, better runners):
   - `x86_64-unknown-linux-gnu` — ubuntu-latest
   - `aarch64-unknown-linux-gnu` — **ubuntu-24.04-arm native** (drops the
     gcc-aarch64 cross setup and the `vendored-openssl` flag from CI; the
     Cargo feature stays for local cross builds)
   - `x86_64-apple-darwin` / `aarch64-apple-darwin` — macos-latest
   - `x86_64-pc-windows-msvc` — windows-latest
4. **Hygiene from Vectorizer**: `CARGO_INCREMENTAL: "0"`, independent
   jobs (no fail-fast coupling), nightly toolchain per rust-toolchain.toml.
5. **Docs**: RELEASE_PROCESS.md release-flow + asset-name table refresh
   (taiki-e names artifacts `<bin>-<target>.tar.gz|zip`).

Docker publishing intentionally stays operator-triggered via
`scripts/docker-publish.*` (phase15 decision) — this task covers GitHub
Release binaries only.

## Impact

- Affected specs: `specs/release-ci/spec.md` (new)
- Affected code: `.github/workflows/release.yml`, `docs/RELEASE_PROCESS.md`
- Breaking change: NO (artifact names change, but current artifacts were
  never published to releases in the first place)
- User benefit: every GitHub Release carries installable binaries + sha256
  for Linux x64/arm64, macOS x64/arm64 and Windows x64

## Reference

- `e:/HiveLLM/Nexus/.github/workflows/release-server.yml`
- `e:/HiveLLM/Vectorizer/.github/workflows/release-artifacts.yml`
