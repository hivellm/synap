# Proposal: phase5_v1.3-dependency-updates

Source: https://github.com/hivellm/synap/pulls (open Dependabot PRs)

## Why

The 1.3.0 release should ship on current dependencies. Five Dependabot PRs are
open, all against `/sdks/typescript`, plus there is a real Thunder version
skew across the SDKs/server that AGENTS.override.md explicitly warns against
("Server and SDK pin the same version so they cannot drift apart"):

Open PRs (as of 2026-07-21):
- **#250** — `@hivehub/thunder` 0.2.1 → **0.2.2** (runtime; the TS wire layer)
- #251 — `@typescript-eslint/eslint-plugin` 8.64.0 → 8.65.0 (dev)
- #248 — `typescript-eslint` 8.64.0 → 8.65.0 (dev)
- #247 — `@typescript-eslint/parser` 8.64.0 → 8.65.0 (dev)
- #249 — `prettier` 3.9.5 → 3.9.6 (dev)

Thunder skew to reconcile:
- workspace `Cargo.toml` pins `thunder-rpc = 0.2.1`
- `sdks/rust/Cargo.toml` still declares `thunder-rpc = 0.2.0` (resolves to 0.2.1
  today via the caret range, but the declared floor drifts from the workspace)
- `sdks/typescript` `@hivehub/thunder` at `^0.2.1`, PR #250 wants 0.2.2
- `sdks/csharp` `HiveLLM.Thunder` at 0.2.1

Only #250 touches runtime behavior; the other four are lint/format tooling.

## What Changes

1. **Runtime — Thunder.** Adopt `@hivehub/thunder` 0.2.2 in the TS SDK (PR #250).
   Check whether the Rust `thunder-rpc` and C# `HiveLLM.Thunder` 0.2.2 exist on
   their registries; if so, align server + Rust SDK + C# SDK to 0.2.2 so all
   wire layers move together (honoring the "no drift" rule). If 0.2.2 is
   TS-only for now, at minimum realign `sdks/rust/Cargo.toml` from 0.2.0 to the
   workspace `thunder-rpc` version to remove the declared-floor drift. The wire
   is frozen v1, so a Thunder patch bump must remain protocol-compatible —
   verify with the SDK interop tests, not just a build.
2. **Dev tooling (TS).** Adopt the four dev-dependency bumps (#247, #248, #249,
   #251). Run `npm run lint` + `npm run format:check` (or equivalents) after, as
   an eslint/prettier bump can surface new lint findings that must be fixed, not
   suppressed.
3. Prefer applying via the Dependabot PRs where CI is green; where a bump
   changes lockfiles the repo does not track (Cargo.lock is gitignored here),
   apply the manifest change directly and let CI regenerate.

## Impact
- Affected specs: none (dependency maintenance)
- Affected code: `sdks/typescript/package.json` (+ lockfile), and if Thunder
  aligns cross-SDK: `Cargo.toml`, `sdks/rust/Cargo.toml`,
  `sdks/csharp/src/Synap.SDK/Synap.SDK.csproj`
- Breaking change: NO — patch/minor bumps only; wire stays v1-frozen (must be
  verified by interop tests)
- User benefit: 1.3.0 ships on current, audited dependencies with a single
  aligned Thunder version instead of a drifting floor
