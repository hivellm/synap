## 1. Implementation
- [ ] 1.1 Add a queue restore primitive that inserts a message with its original ID, priority, retry counters, delivery count and timestamps
- [ ] 1.2 Use the restore primitive for WAL replay of `QueuePublish`, snapshot restore and replica apply
- [ ] 1.3 Apply replayed ACK and NACK by message ID in the ready or pending state, tolerating only not-found and failing recovery on any other error
- [ ] 1.4 Test WAL-only and snapshot+WAL recovery after ACK, ID stability of unacknowledged messages, and master/replica ID parity through publish, consume and ACK

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
