# Proposal: phase5_typescript-7-migration

## Why

The 1.3.1 dependency sweep (phase3) upgraded every TypeScript SDK dependency
that could move, with one exception: TypeScript itself is pinned at 6.0.3 while
7.0.2 is published. The blocker is not our code — `tsc --noEmit`, the build and
the 404-test suite all pass — it is `typescript-eslint` 8.66, whose
`@typescript-eslint/eslint-plugin` and `@typescript-eslint/parser` both declare
`peer typescript ">=4.8.4 <6.1.0"`. Installing TypeScript 7 today means running
the linter against an unsupported compiler, which is exactly the configuration
typescript-eslint warns produces wrong results. The upgrade therefore has to
wait for a typescript-eslint release that admits TypeScript 7 — and it needs a
task so it does not silently rot the way the five Dependabot PRs did.

## What Changes

- Bump `typescript` to ^7.x in `sdks/typescript` once `typescript-eslint`
  publishes a release whose peer range accepts it, bumping the three
  typescript-eslint packages in the same step.
- Re-run type-check, lint, the unit suite and the S2S suite, and fix whatever
  TypeScript 7's stricter checking surfaces.
- Consider the same bump for the GUI (`gui/package.json`, currently TypeScript
  5.7) so the repository runs one compiler major.

## Impact

- Affected specs: none (toolchain upgrade, no behavior spec)
- Affected code: `sdks/typescript/package.json`, possibly SDK sources that
  TypeScript 7 rejects, `gui/package.json`
- Breaking change: NO for SDK consumers — the published `.d.ts` surface is
  unchanged unless TypeScript 7 forces a signature fix, which this task treats
  as a finding to report rather than a silent change.
- User benefit: the SDK is compiled by a supported, current toolchain instead
  of drifting one major behind.
