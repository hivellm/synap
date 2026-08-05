## 1. Core

- [x] 1.1 Add `created_at` and `generation` to `RoomStats` and stamp them in `Room::new`
- [x] 1.2 Add a process-global strictly-monotonic generation source seeded from the wall clock
- [x] 1.3 Unit tests: generation changes on room recreation, is strictly increasing, and is stable while the room lives

## 2. Protocols

- [x] 2.1 Expose `created_at`/`generation` in RESP3 `SSTATS`
- [x] 2.2 Expose `created_at`/`generation` in SynapRPC `SSTATS`
- [x] 2.3 Protocol tests covering both native `SSTATS` payloads

## 3. SDKs

- [x] 3.1 Rust SDK `StreamStats`: add `created_at`/`generation`
- [x] 3.2 TypeScript `StreamStats`: realign with the server payload and add the new fields

## 4. Tail (docs + tests)

- [x] 4.1 Update or create documentation covering the implementation
- [x] 4.2 Write tests covering the new behavior
- [x] 4.3 Run tests and confirm they pass
