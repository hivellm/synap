# Proposal: phase10_typescript-7-swap

## Why

phase5 established that the TypeScript SDK and the GUI already compile clean
under TypeScript 7 — both carry a `type-check:next` script that proves it on
every run — and that the only thing standing in the way is the linter.
`@typescript-eslint/typescript-estree` is built on the JavaScript compiler API,
which the `typescript` 7.x package no longer exposes:

```
node -e "const ts=require('typescript'); console.log(ts.version, typeof ts.createProgram)"
7.0.2 undefined
```

phase5 is archived because everything it could deliver is delivered. This task
carries the one step that depends on a third party, so it does not get lost in
an archived checklist.

## What Changes

- Bump `typescript` to 7.x and the three typescript-eslint packages together in
  `sdks/typescript`, once a typescript-eslint release accepts it.
- Drop the `typescript-next` alias and fold `type-check:next` back into
  `type-check`; the explicit `node node_modules/typescript/bin/tsc` paths in the
  scripts can go back to the bare `tsc` at the same time.
- Same swap for the GUI, which is on TypeScript 5.7 today.
- Re-run lint, build, the unit suite and the S2S suite, and fix whatever
  TypeScript 7's checking surfaces that 6 let through.

## Impact

- Affected specs: none (toolchain upgrade)
- Affected code: `sdks/typescript/package.json`, `gui/package.json`, and any
  source TypeScript 7 rejects
- Breaking change: NO — the published `.d.ts` surface is unchanged unless
  TypeScript 7 forces a signature fix, which this task reports rather than
  applying silently.
- User benefit: the SDK ships compiled by a supported, current toolchain, and
  the repository stops carrying two compilers.

## Unblocks when

```
npm view @typescript-eslint/parser@latest peerDependencies
```

reports a `typescript` range that admits 7.x. As of 2026-08-06 it is
`>=4.8.4 <6.1.0`, in 8.66.0 and in the 8.66.1 canary alike.
