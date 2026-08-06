## 1. Upgrade

- [x] 1.1 Establish what actually blocks TypeScript 7 (not just the declared peer range)
- [ ] 1.2 Bump `typescript` and the three typescript-eslint packages together in the TypeScript SDK — blocked upstream, see below
- [x] 1.3 Fix everything TypeScript 7 rejects in the SDK sources
- [x] 1.4 Make the GUI compile under TypeScript 7 (`baseUrl` was removed in 7.0)

## 2. Verification

- [x] 2.1 Type-check the SDK with both compilers, lint and build it
- [x] 2.2 Run the unit suite and the S2S suite against a live server
- [x] 2.3 Build the GUI

## 3. Tail (docs + tests)

- [x] 3.1 Update or create documentation covering the implementation
- [x] 3.2 Write tests covering the new behavior
- [x] 3.3 Run tests and confirm they pass

## Blocker for 1.2

The `typescript` 7.x package no longer exposes the JavaScript compiler API:

```
node -e "const ts=require('typescript'); console.log(ts.version, typeof ts.createProgram)"
7.0.2 undefined
```

`createProgram`, `createSourceFile`, `ScriptTarget` and `forEachChild` are all
absent, and `@typescript-eslint/typescript-estree` is built on exactly those.
So the peer range (`>=4.8.4 <6.1.0`, unchanged in 8.66.0 and in the 8.66.1
canary) is describing a real incompatibility, not a stale constraint —
installing TypeScript 7 with `--legacy-peer-deps` would leave the lint step
broken rather than merely warned about.

Unblocks when typescript-eslint ships a release built against whatever API
TypeScript 7 exposes for tools. The check is one command:

```
npm view @typescript-eslint/parser@latest peerDependencies
```

Both `type-check:next` scripts already prove the sources are ready, so that
release is the only thing left.
