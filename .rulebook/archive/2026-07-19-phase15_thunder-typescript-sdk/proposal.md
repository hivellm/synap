# Proposal: phase15_thunder-typescript-sdk

Source: https://github.com/hivellm/thunder — `docs/analysis/04-adoption-plan.md` (P3).

## Why

`sdks/typescript/src/transports/synap-rpc.ts` is one of the 15 hand-ported SDK
transports Thunder exists to replace. Thunder's inventory found three different
msgpack libraries in use across the family's TypeScript transports, and 9 of the 15
transports allocating from an untrusted length prefix with no cap — a remote client
can make them allocate whatever a 4-byte prefix claims.

`@hivehub/thunder` 0.1.1 is published on npm and gives the TypeScript SDK the same
uniform floor every other language gets: demux by id, cap on encode and decode,
connect + per-call timeouts, lazy reconnect, push hook, typed error parsing.

## What Changes

- `sdks/typescript/package.json` adds `@hivehub/thunder` and drops the direct
  msgpack dependency the local transport used.
- `src/transports/synap-rpc.ts` becomes a thin adapter over Thunder's `Client`,
  constructed with the Synap `Config` (scheme `synap`, port 15501,
  `AuthCommand` handshake, push enabled, `[CODE]`/RESP3 error superset).
- Value conversion between Synap's SDK types and Thunder's `Value` is centralized
  in one module; the `Bytes` handling must decode both MessagePack `bin` (what the
  server emits after phase12) and the legacy int-array form.
- `src/pubsub.ts` consumes push frames from Thunder's push hook.
- `src/transports/command-map.ts` and the public client surface are untouched —
  command catalogs are Synap's, transport is Thunder's.

## Impact
- Affected specs: `.rulebook/tasks/phase15_thunder-typescript-sdk/specs/typescript-sdk-transport/spec.md`
- Affected code: `sdks/typescript/src/transports/`, `sdks/typescript/src/pubsub.ts`, `sdks/typescript/package.json`
- Breaking change: NO — the exported client API is unchanged.
- User benefit: frame cap enforced (closes the unbounded-allocation gap), real
  pipelining, enforced timeouts, and one vetted msgpack implementation.
