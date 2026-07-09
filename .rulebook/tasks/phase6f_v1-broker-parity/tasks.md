## 1. Implementation
- [ ] 1.1 Add optional disk-segment spill for streams (or bound retention by min committed offset) so unread events are not dropped
- [x] 1.2 Emit an explicit drop/lag signal when stream retention forces a drop (RoomStats.dropped counts evictions unread by slowest subscriber — commit ec462f1)
- [x] 1.3 Index stream consume by offset (index = offset - min_offset) for O(1) seek (commit a39ae03)
- [ ] 1.4 Model queue consumers explicitly with a per-consumer prefetch window
- [ ] 1.5 Implement fair round-robin dispatch across queue consumers and report the real consumer count (real consumer count done via in-flight tracking, commit f5e5dc4; fair round-robin dispatch remains)
- [x] 1.6 Replace the global 1s ACK-deadline sweep with a per-queue deadline heap/timer-wheel (commit eb5672e)
- [ ] 1.7 Add config knobs (stream spill, queue prefetch) with safe defaults
- [ ] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (streams/queues guides)
- [ ] 2.2 Write tests covering the new behavior (no silent drop of unread stream events; O(1) consume seek; multi-consumer prefetch fairness; deadline expiry without global stall)
- [ ] 2.3 Run tests and confirm they pass
