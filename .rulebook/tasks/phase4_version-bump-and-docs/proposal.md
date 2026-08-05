# Proposal: phase4_version-bump-and-docs

## Why

Synap ships one version number across seven publishable artifacts — the Rust
workspace (server, CLI, migrate, core), the Rust SDK, and the TypeScript,
Python, PHP and C# SDKs — plus the GUI. They are only in sync if someone bumps
them together, and a client that reports a different version than the server it
talks to makes every support conversation start with an archaeology session.
1.3.1 also changes the documented `stream.stats` payload (phase1) and the
dependency set (phase2/phase3), so the docs that describe both are stale the
moment those land. Bumping and documenting is the closing act of the release,
after the code is final, not something to retrofit afterwards.

## What Changes

- Version bumped to 1.3.1 in every manifest: the Cargo workspace, the
  TypeScript `package.json`, the Python `pyproject.toml`, the PHP
  `composer.json`, the C# `.csproj`, and the GUI `package.json`, plus any
  version constant embedded in source or reported over the wire.
- `CHANGELOG.md` gets a `1.3.1` section covering the wipe discriminator, the
  dependency upgrades and the audit result.
- Documentation refreshed where 1.3.1 changed it: the `stream.stats` payload in
  the REST API reference and the OpenAPI documents, the stream consumer guide
  (how to use `generation` to detect a wipe), the SDK docs for the SDKs whose
  typed stats surface changed, and any README version badge or install snippet
  that pins a version.

## Impact

- Affected specs: none (release mechanics)
- Affected code: `Cargo.toml`, `sdks/typescript/package.json`,
  `sdks/python/pyproject.toml`, `sdks/php/composer.json`,
  `sdks/csharp/**/*.csproj`, `gui/package.json`, `CHANGELOG.md`, `README.md`,
  `docs/api/REST_API.md`, `docs/api/openapi.{json,yml}`,
  `docs/users/streams/*.md`, `docs/users/sdks/*.md`
- Breaking change: NO
- User benefit: every artifact reports the same 1.3.1, and the documentation
  describes the payload and dependency set that actually ships.
