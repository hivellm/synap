# Proposal: phase12_replace-serde-yaml

## Why

`serde_yaml` 0.9 is deprecated (it publishes as `0.9.34+deprecated`) and pulls
in `unsafe-libyaml`, which the RustSec database flagged as unmaintained on
2026-10-12 (RUSTSEC-2024-0449, no patched release). That advisory turned the
CI lint job's `cargo deny check advisories` red on every branch. 1.3.2 put it
on the `deny.toml` ignore list with a rationale so releases are not blocked,
but the ignore only defers the problem: the server's configuration parser sits
on an archived C-to-Rust translation of libyaml that will not get fixes.

## What Changes

- Replace `serde_yaml` with a maintained, serde-compatible YAML crate in the
  workspace, `synap-server` (which still declares its own `serde_yaml = "0.9"`
  instead of the workspace entry) and `synap-migrate`.
- Port the call sites: `ServerConfig` loading in
  `crates/synap-server/src/config.rs`, the config unit tests there, and the
  round-trip in `crates/synap-server/tests/config_tests.rs`.
- Prove every shipped config file (the eight under `config/`) still parses to
  the same `ServerConfig`, including defaults, before and after the swap.
- Drop RUSTSEC-2024-0449 from the `deny.toml` ignore list.

## Impact

- Affected specs: none (parser swap behind the same config schema)
- Affected code: `Cargo.toml`, `crates/synap-server/Cargo.toml`,
  `crates/synap-migrate/Cargo.toml`, `crates/synap-server/src/config.rs`,
  `crates/synap-server/tests/config_tests.rs`, `deny.toml`
- Breaking change: NO, as long as YAML 1.1 quirks the old parser accepted
  (unquoted `yes`/`no`, octal-looking numbers) are confirmed to resolve the same
  way, or reported if they do not
- User benefit: the configuration parser is maintained again and the advisory
  gate carries no ignore for code Synap runs on every start
