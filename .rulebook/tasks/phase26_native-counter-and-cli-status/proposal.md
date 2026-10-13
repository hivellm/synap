# Proposal: phase26_native-counter-and-cli-status

Source: docs/analysis/next-release-review-2026-10-12/ (findings E-04 and E-05, priority P2)
Target release: 1.4.0

## Why

- **E-04**: `INCR count 10` adds 10 over HTTP and 1 over SynapRPC or RESP3.
  The CLI sends an optional `amount` (`crates/synap-cli/src/main.rs`) and the
  HTTP handler uses it (`server/handlers/kv_cmd.rs`), but the native command
  maps always emit one-argument `INCR`/`DECR`: the Rust SDK
  (`sdks/rust/src/transport/mapping.rs`), the TypeScript SDK
  (`sdks/typescript/src/transports/command-map.ts`) and the other SDK maps
  built from the same table. Switching transport silently changes arithmetic.
  The CLI treats a malformed amount as 1.
- **E-05**: `synap-cli ... PING` turns every connection error into
  `Ok("Server unreachable")` (`crates/synap-cli/src/main.rs`), so the process
  exits 0 when the server is down or the credentials are rejected. Health
  scripts that check the exit status report a healthy server.

## What Changes

- Every SDK native map (Rust, TypeScript, Python, C#, PHP) sends `INCRBY` or
  `DECRBY` when an amount is given and keeps `INCR`/`DECR` without one, with
  the same validation and overflow behavior as HTTP.
- The CLI rejects a malformed amount before sending anything.
- `PING` uses a protocol ping, exits non-zero on connection refusal, timeout
  or denied credentials, and writes the diagnostic to stderr.

## Impact

- Affected specs: sdk (counter mapping), cli (exit status)
- Affected code: `sdks/rust/src/transport/mapping.rs`,
  `sdks/typescript/src/transports/command-map.ts`,
  `sdks/python/synap_sdk/command_map.py`,
  `sdks/csharp/src/Synap.SDK/CommandMapper.cs`,
  `sdks/php/src/CommandMapper.php`, `crates/synap-cli/src/main.rs`
- Dependencies: none
- Breaking change: YES for scripts that relied on `PING` exiting 0 while the
  server is down.
- User benefit: counters behave the same on every transport, and CLI health
  checks report failures.
