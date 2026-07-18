# Proposal: phase19_thunder-release-1-1-0

Source: https://github.com/hivellm/thunder — Gate G2 / G3 criteria in
`docs/analysis/04-adoption-plan.md`.

## Why

phase12–phase18 swap the server and every SDK with a Thunder counterpart onto the
shared protocol. Individually each one passes its own suite; what none of them
proves is the thing that actually matters to a user: that a Thunder-based Synap
server and every Synap SDK — including the two with no Thunder package (PHP, Java)
and any pre-Thunder client still in the wild — still talk to each other correctly.

That cross-language verification is Thunder's Gate G2/G3, and it is the gate for
tagging 1.1.0. A release only happens if it is green in every cell; a red cell
becomes an issue on `hivellm/thunder` (if the fault is Thunder's) or a fix in this
repo (if it is Synap's), and the release waits.

## What Changes

- A cross-SDK interop matrix is run against one Thunder-based server build: each of
  the 7 SDKs (rust, typescript, python, csharp, go, php, java) completes
  authenticate → SET/GET with a binary value → SUBSCRIBE/PUBLISH → an error
  round-trip.
- The legacy-client cell is explicit: a pre-Thunder SDK build (int-array `Bytes`,
  no cap) is exercised against the new server to prove the tolerance path.
- PHP and Java have no Thunder package. They keep their hand-written transports;
  this task only proves they still interoperate, and files the packaging gap
  upstream if the family intends to cover them.
- Version bump to 1.1.0 across the workspace and every SDK manifest.
- `CHANGELOG.md` gets the 1.1.0 section, including the `Bytes` canonicalization and
  the `synap-protocol` deprecation, with a migration note.
- The terminal `synap-protocol` shim prepared in phase13 is published.
- Release tag and artifacts.

## Impact
- Affected specs: `.rulebook/tasks/phase19_thunder-release-1-1-0/specs/release-verification/spec.md`
- Affected code: `Cargo.toml`, every SDK manifest, `CHANGELOG.md`, `README.md`, `docs/`
- Breaking change: NO for wire consumers. YES for Rust consumers of
  `synap-protocol` (deprecated shim).
- User benefit: a verified, single-protocol release across seven SDKs, with the
  interop matrix as evidence rather than assertion.
