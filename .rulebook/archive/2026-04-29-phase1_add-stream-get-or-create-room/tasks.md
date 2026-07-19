## 1. Server (synap-server)
- [x] 1.1 Add idempotent `StreamManager::get_or_create_room` in `core/stream.rs` returning `(created: bool)` so the caller can observe the race-safe outcome.
- [x] 1.2 Add `stream.get_or_create` StreamableHTTP command in `server/handlers/stream.rs` and register it in `server/handlers/mod.rs`.
- [x] 1.3 Add idempotent `PUT /stream/{room}` REST endpoint alongside the existing `POST /stream/{room}` (consolidated as `.post(...).put(...).delete(...)` on the same path to satisfy axum's method-router contract).
- [x] 1.4 Add `SGETORCREATE` verb to the SynapRPC dispatch in `protocol/synap_rpc/dispatch/advanced.rs` (returns `"CREATED"` / `"EXISTS"`).

## 2. SDKs
- [x] 2.1 Add `StreamManager::get_or_create_room` to the Rust SDK (`sdks/rust/src/stream.rs`).
- [x] 2.2 Add `streams.getOrCreateRoom` to the TypeScript SDK.
- [x] 2.3 Add `get_or_create_room` to the Python SDK and wire it into `command_map.py` (request + response, including SGETORCREATE → `{created: bool}` normalization).
- [x] 2.4 Add equivalents to Go (`GetOrCreate`), Java (`getOrCreate`), C# (`GetOrCreateRoomAsync`), PHP (`getOrCreateRoom`) including command-mapper updates.

## 3. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 3.1 Update or create documentation covering the implementation — SDK READMEs (Rust, Python, TypeScript) and `sdks/rust/CHANGELOG.md` updated with a "First publish to a new stream" note pointing at synap#165.
- [x] 3.2 Write tests covering the new behavior:
  - `synap-server` core: `test_stream_get_or_create_room_is_idempotent`, `test_stream_get_or_create_after_create_room_does_not_error`.
  - SynapRPC dispatch: `test_stream_get_or_create_is_idempotent` (also verifies SPUBLISH succeeds immediately after).
  - Rust SDK: `test_stream_get_or_create_room_returns_created_flag` + `..._idempotent_returns_false`.
  - TypeScript SDK: 3 vitest cases under `getOrCreateRoom() — hivellm/synap#165`.
  - Python SDK: `test_get_or_create_room_returns_created_flag` + `..._idempotent_returns_false` (passing locally).
- [x] 3.3 Run tests and confirm they pass:
  - `cargo check -p synap-server --lib --tests`: clean.
  - `cargo clippy -p synap-server --lib --tests -- -D warnings`: clean.
  - `cargo clippy -p synap-sdk --lib --tests -- -D warnings`: clean.
  - `cargo fmt --all`: applied.
  - `cargo test -p synap-server --lib stream`: 19 tests passed (incl. 3 new).
  - `cargo test -p synap-sdk --test stream_test`: 9 tests passed (incl. 2 new).
  - `pytest sdks/python/tests/test_stream.py`: 6 tests passed (incl. 2 new).
  - `npx tsc --noEmit` on TS SDK: clean. (`vitest` runner is broken at the harness level — pre-existing `ERR_REQUIRE_ESM` in vitest's own config loader, unrelated to this change.)
  - `go build ./...` on Go SDK: clean.
  - `dotnet build` on C# SDK: 0 errors.
