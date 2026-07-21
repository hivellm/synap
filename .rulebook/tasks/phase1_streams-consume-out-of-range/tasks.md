## 1. Implementation
- [x] 1.1 Add `SynapError::StreamOffsetOutOfRange { requested, earliest }` in `core/error.rs` with a HTTP 400 mapping
- [x] 1.2 Change `Room::consume()` to return `Result<Vec<StreamEvent>, SynapError>`, erroring when `from_offset < min_offset && min_offset > 0`
- [x] 1.3 Propagate the `Result` through the stream store's public `consume` API (flattened to `String` at the store boundary to preserve existing REST/RESP3/SynapRPC caller signatures)
- [x] 1.4 Return the 400 (with `earliest`) from `server/handlers/stream.rs` (via the existing `map_err(InvalidRequest)`, message carries `earliest`)

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (doc-comments on `Room::consume`/`StreamStore::consume` + CHANGELOG [Unreleased] Fixed)
- [x] 2.2 Write tests covering the new behavior (evicted-offset consume errors; in-range and new-room consume unaffected; error status/display)
- [x] 2.3 Run tests and confirm they pass (`cargo check` ✓ + `clippy -D warnings` ✓ + 436 core tests ✓)
