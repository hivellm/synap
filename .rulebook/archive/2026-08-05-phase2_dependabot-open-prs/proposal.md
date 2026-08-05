# Proposal: phase2_dependabot-open-prs

## Why

Five Dependabot pull requests are open against `main` and none of them can be
merged as-is: they were each branched from a different `main` and two of them
are semver-major bumps that the compiler has never seen built together
(`rmcp` 2.1.0 → 3.0.1 and `base64` 0.22 → 0.23). Leaving them open means the
1.3.1 fix release ships on dependency versions that already have published
successors, and the two major bumps keep rotting until they need a migration
nobody has scoped. Applying all five on the release branch — with the full
quality gate run once, over the combined set — resolves every open PR in a
single verified state instead of five unverified merges.

## What Changes

- Rust workspace: `base64` 0.22 → 0.23 (workspace dependency and the Rust SDK's
  own pin) and `rmcp` 2.1.0 → 3.0.1 in `synap-server`, adapting call sites to
  whatever the majors changed.
- TypeScript SDK dev dependencies: `msgpackr` → ^2.0.5, `@types/node` →
  ^26.1.2, `eslint` → ^10.8.0.
- `Cargo.lock` and `sdks/typescript/package-lock.json` refreshed accordingly.
- The five Dependabot PRs (#252, #253, #254, #255, #256) close when the release
  branch carrying the same bumps lands on `main`.

## Impact

- Affected specs: none (dependency maintenance, no behavior spec)
- Affected code: `Cargo.toml`, `sdks/rust/Cargo.toml`,
  `crates/synap-server/Cargo.toml`, any `base64`/`rmcp` call sites,
  `sdks/typescript/package.json`
- Breaking change: NO — both majors are internal dependencies; the wire
  protocols, REST surface and SDK APIs are unchanged.
- User benefit: 1.3.1 ships on current dependencies with the MCP server and the
  base64 codepaths verified against the new majors, instead of accumulating
  five stale upgrade PRs.
