# Proposal: phase13_ts-e2e-native-parity

## Why

The TypeScript SDK's `e2e.test.ts` (gated on `RUN_E2E=true`) had not run since
1.0.0: it read `config.yml` from the repository root after the file moved to
`config/`. 1.3.2 fixed the harness, and its first run fails 9 of 22 tests.
At least one failure is a real SDK bug: over SynapRPC and RESP3,
`queue.listQueues()` and `stream.listRooms()` return `undefined`, because the
native response mapper returns the bare array for `queue.list`/`stream.list`
while both methods read `result.queues`/`result.rooms` from the HTTP envelope.

## What Changes

- Wrap the native `queue.list`/`stream.list` replies as `{ queues }`/`{ rooms }`
  in `src/transports/command-map.ts`, with a transport-level unit test.
- Find out why Lua scripting fails on all three transports
  (`TypeError: Cannot read properties of undefined (reading '0')`), and fix
  the SDK or the test, whichever is wrong.
- The `UnsupportedCommandError` regression tests use `bitmap.setbit`, which
  the native transports now map. Point them at a command that is still
  unmapped, or retire them if every command is mapped.
- Run the E2E suite green on all three transports.

## Impact

- Affected specs: none
- Affected code: `sdks/typescript/src/transports/command-map.ts`,
  `sdks/typescript/src/__tests__/e2e.test.ts`, possibly the script module
- Breaking change: NO
- User benefit: listing queues and stream rooms works on the native transports,
  and the cross-transport E2E suite becomes a usable release gate again
