# Proposal: phase3_dependency-audit-all-sdks

## Why

The five open Dependabot PRs (phase2) only cover what Dependabot happens to
watch: the Rust workspace and the TypeScript SDK. The Python, PHP and C# SDKs
have no bot coverage at all, so nobody knows how far behind their pinned
dependencies are or whether any of them carries a published advisory. A fix
release is the right moment to sweep every manifest in the repository — server
and all five SDKs — audit it for vulnerabilities, and take every upgrade that
does not require an API migration, so 1.3.1 ships on a dependency set that has
actually been checked rather than one that has merely not been looked at.

## What Changes

- Audit every manifest for known advisories: `cargo audit` (workspace + Rust
  SDK), `npm audit` (TypeScript SDK, GUI), `pip-audit` (Python SDK),
  `composer audit` (PHP SDK), `dotnet list package --vulnerable` (C# SDK).
- Upgrade every dependency that can move without an API migration, per
  ecosystem, and record the ones deliberately held back with the reason.
- Re-run each SDK's own quality gate (type-check/lint/test) after its upgrade
  so nothing lands unverified.
- Report the audit result — advisories found, upgrades applied, upgrades
  deferred — in the release notes for 1.3.1.

## Impact

- Affected specs: none (dependency maintenance, no behavior spec)
- Affected code: `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`,
  `sdks/rust/Cargo.toml`, `sdks/typescript/package.json`,
  `sdks/python/pyproject.toml`, `sdks/php/composer.json`,
  `sdks/csharp/**/*.csproj`, `gui/package.json`
- Breaking change: NO — upgrades requiring an API migration are deferred to a
  follow-up task rather than forced into a patch release.
- User benefit: every shipped SDK is known to be free of published advisories,
  and the ecosystems with no bot coverage stop silently rotting.
