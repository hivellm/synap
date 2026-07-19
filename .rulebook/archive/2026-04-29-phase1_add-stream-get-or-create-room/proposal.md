# Proposal: phase1_add-stream-get-or-create-room

Closes hivellm/synap#165.

## Why

Publishing to a Synap stream room that does not exist returns
`Server error: Invalid request: Room '<name>' not found`. This is
correct server behavior, but every client integration ends up
reimplementing the same defensive wrapper:

> try `streams.publish(room, ...)`. If the error contains `not found`
> and `Room`, call `streams.create_room(room, None)` and retry once.

The Cortex project alone now carries this boilerplate in three crates
(`cortex-bootstrap`, `cortex-classifier-worker`, `cortex-ingestion`)
after a 2026-04-28 silent-loss incident. Each implementation is
slightly different, the failure mode is silent until the raw error
string is inspected, and a new crate that forgets to add the wrapper
will drop events on first publish.

The fix is to make Synap's "create" idempotent and to expose a
`get_or_create_room` operation in the wire protocol and every SDK,
mirroring the well-known idempotent pattern from `redis-py`,
GCP `pubsub.create_topic`, and AWS `kinesis CreateStream`.

## What Changes

1. Server (`synap-server`):
   - `core::stream::StreamManager::get_or_create_room(name, max_events)`
     idempotent: returns whether the room was newly created and never
     errors when it already exists.
   - New StreamableHTTP command `stream.get_or_create` that wraps it.
   - New REST endpoint `PUT /stream/{room}` (idempotent) in addition
     to the existing non-idempotent `POST /stream/{room}`.
   - New SynapRPC verb `SGETORCREATE`.
2. Rust SDK (`sdks/rust`):
   - `StreamManager::get_or_create_room(name, max_events)`.
3. TypeScript SDK (`sdks/typescript`):
   - `streams.getOrCreateRoom(name, opts)` mirroring the Rust API.
4. Other SDKs (Python, Go, Java, C#, PHP):
   - Add `get_or_create_room` (or language-idiomatic equivalent)
     where a stream client already exists, to keep parity.
5. Documentation:
   - Update SDK READMEs with a "First publish to a new stream" note.
   - Add a migration tip pointing at the deprecated retry-and-create
     boilerplate it replaces.

## Impact

- Affected specs: stream subsystem (wire protocol, REST, RPC),
  multi-language SDK surface.
- Affected code:
  - `synap-server/src/core/stream.rs`
  - `synap-server/src/server/handlers/{mod.rs,stream.rs}`
  - `synap-server/src/server/router.rs`
  - `synap-server/src/protocol/synap_rpc/dispatch/advanced.rs`
  - `sdks/rust/src/stream.rs`
  - `sdks/typescript/src/streams.*`
  - other SDKs as listed above
- Breaking change: NO. Existing `stream.create` semantics are
  preserved; the new command/endpoint is additive.
- User benefit: removes a recurring foot-gun (silent first-publish
  failure on fresh rooms), collapses ~25 lines of per-client
  retry-and-create boilerplate into a single SDK call, and gives the
  same idempotent guarantees the rest of Synap's create operations
  already have.
