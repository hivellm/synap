# Proposal: phase11_v1-3-2-dependabot-fixes

## Why

Eleven Dependabot pull requests are open against `main` (#266, #269, #281,
#283-#290): one Cargo requirement bump (`zstd` 0.13 -> 0.14) and ten dev or
runtime dependency bumps in the TypeScript SDK, two of them majors (`vitest`
and `@vitest/coverage-v8` 4 -> 5). Each PR touches the same three lockfiles in
`sdks/typescript`, so merging them one by one would conflict on every merge
after the first. They are applied together on a `release/v1.3.2` branch, the
same way 1.3.1 absorbed #252-#256, and shipped as the 1.3.2 fix release.

There are no open Dependabot security alerts; this is dependency hygiene, not
an advisory response.

## What Changes

- Root `Cargo.toml`: `zstd` 0.13 -> 0.14 (used by `synap-core`'s compressor
  and `synap-server`).
- `sdks/typescript`: `prettier` 3.9.8, `eslint` 10.11.0, `vitest` and
  `@vitest/coverage-v8` 5.0.1, `@types/node` 26.6.2, `typescript-eslint`,
  `@typescript-eslint/parser` and `@typescript-eslint/eslint-plugin` 8.70.0,
  `msgpackr` 2.1.0, `uuid` 14.0.2; `package-lock.json` and `pnpm-lock.yaml`
  regenerated.
- Every publishable artifact moves to 1.3.2: Cargo workspace, TypeScript,
  Python and C# SDKs, and the desktop GUI.
- CHANGELOG `[1.3.2]` section; README version badge.

TypeScript 7 stays out: typescript-eslint 8.70.0 still declares
`typescript >=4.8.4 <6.1.0`, so `phase10_typescript-7-swap` remains blocked.

## Impact

- Affected specs: none (dependency and version bump)
- Affected code: `Cargo.toml`, `sdks/typescript/package.json` and lockfiles,
  version fields in every SDK manifest and `gui/package.json`
- Breaking change: NO
- User benefit: current dependencies across the server and the TypeScript SDK,
  and an empty Dependabot queue
