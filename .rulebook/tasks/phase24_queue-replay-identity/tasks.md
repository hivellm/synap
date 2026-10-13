## 1. Implementation
- [x] 1.1 Add a queue restore primitive that inserts a message with its original ID, priority, retry counters, delivery count and timestamps — `QueueManager::restore_message` (exact message, publish position, no depth check) plus `dump_state`/`restore_queue` (`QueueState`: config, ready, in-flight in delivery order, dead letters); `QueueMessageRecord` keeps `created_at`, which `QueueMessage` serde drops. Synap has no separate delivery counter: `retry_count` counts redeliveries and is restored
- [x] 1.2 Use the restore primitive for WAL replay of `QueuePublish`, snapshot restore and replica apply — `apply_operation` restores `QueuePublish`/new `QueuePublishFull`; snapshot v5 stores whole queues (v4/v3/v2 still load); new `QueueCreate`/`QueueDelete`/`QueuePurge` records on every transport via `durable`; ACK-deadline requeues recorded as NACKs
- [x] 1.3 Apply replayed ACK and NACK by message ID in the ready or pending state, tolerating only not-found and failing recovery on any other error — `replay_ack`/`replay_nack`/`remove_messages` + `tolerate_not_found` in `apply.rs`
- [ ] 1.4 Test WAL-only and snapshot+WAL recovery after ACK, ID stability of unacknowledged messages, and master/replica ID parity through publish, consume and ACK

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
