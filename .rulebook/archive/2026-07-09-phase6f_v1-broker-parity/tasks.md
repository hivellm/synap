## 1. Implementation
- [x] 1.1 Bound stream retention by min committed consumer offset so unread events are not dropped (protected up to max_unread_buffer_size hard cap — commit 623313f)
- [x] 1.2 Emit an explicit drop/lag signal when stream retention forces a drop (RoomStats.dropped counts evictions unread by slowest subscriber — commit ec462f1)
- [x] 1.3 Index stream consume by offset (index = offset - min_offset) for O(1) seek (commit a39ae03)
- [x] 1.4 Model queue consumers explicitly with a per-consumer prefetch window (prefetch_limit enforced via in-flight map — commit 90ac3c9)
- [x] 1.5 Fair dispatch across queue consumers via prefetch backpressure + real consumer count (commits f5e5dc4, 90ac3c9)
- [x] 1.6 Replace the global 1s ACK-deadline sweep with a per-queue deadline heap/timer-wheel (commit eb5672e)
- [x] 1.7 Add config knobs (max_unread_buffer_size, prefetch_limit) with safe defaults (commits 623313f, 90ac3c9)
- [x] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/broker-retention-and-prefetch.md)
- [x] 2.2 Write tests covering the new behavior (retention hard-cap protection + drop count; O(1) consume seek; prefetch throttle + fair dispatch; deadline expiry without global stall)
- [x] 2.3 Run tests and confirm they pass (full workspace suite: 1708 passed, 0 failed)
