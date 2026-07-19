# Proposal: phase14_image-hardening-sdk-docs

## Why

Docker Scout flags 38 vulnerabilities (1 critical CVE-2026-12087 9.1, 2 high,
3 medium, 28 low) and 114 packages on the freshly published
`hivehub/synap:1.0.0` image. Every one of them comes from the
`dhi.io/debian-base:trixie-dev` runtime base plus the apt-installed
`ca-certificates`/`tzdata`/`wget` — none from Synap itself, whose binary is a
fully static musl build that needs no distro at all. The only reason a distro
was present was the `wget`-based HEALTHCHECK.

Separately, the SDK READMEs/CHANGELOGs lag the 1.0.0 release: version strings
still cite 0.1/0.2.0-beta/0.11.1, no SDK changelog has a 1.0.0 entry (Go and
Java have no changelog at all), and several quick-start examples lead with the
HTTP transport instead of the SynapRPC default.

## What Changes

1. **Native health probe**: `synap-server --health-check` — a plain-std HTTP
   GET against `/health` (SYNAP_HEALTH_ADDR override), exiting 0/1, so the
   container HEALTHCHECK can run the server binary itself with no shell/wget.
2. **Runtime image → `FROM scratch`**: a `rootfs` prep stage assembles the
   passwd/group entries, CA bundle and /data tree; the final image carries only
   the static binary + config + CA certs. Zero distro packages ⇒ zero distro
   CVE surface. Non-root (UID 1000) preserved.
3. Rebuild, smoke test (health + RESP3/RPC), re-push `1.0.0` + `latest`.
4. **SDK docs review (all 7)**: add a `## [1.0.0]` entry to every SDK
   CHANGELOG (create Go/Java changelogs), documenting the audit fixes (TS
   object serialization, Python dead KV endpoint) and dependency bumps; fix
   stale version strings in READMEs; lead quick-start examples with the
   `synap://` default transport.

## Impact

- Affected specs: none
- Affected code: crates/synap-server/src/main.rs (health probe), Dockerfile,
  sdks/*/README.md, sdks/*/CHANGELOG.md
- Breaking change: NO (image behaviour identical; no shell inside the container
  is the only observable difference)
- User benefit: a clean Scout/vulnerability report for the official image and
  accurate, release-ready SDK documentation
