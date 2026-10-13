## 1. Implementation
- [x] 1.1 Stores: a `clear()` on the list, set, sorted-set, HyperLogLog and bitmap stores, and `HashStore::clear` available outside tests, so a replica can drop its state before loading an image
- [x] 1.2 Replication log: `MasterNode::replicate` assigns the offset, appends to the log and queues the record for fan-out in one critical section (replicas receive records in offset order)
- [x] 1.3 Shared barrier: the master node owns the `WriteOrder`; the persistence layer built with a master uses it; `MasterNode::with_stores` takes every datatype store; `main.rs` builds the master after every store exists
- [x] 1.4 Versioned full-sync image: `ReplicationCommand::FullSyncImage { version, offset, checksum, image }` with every datatype (KV + absolute expiry, hashes, lists, sets, sorted sets, HLL, bitmaps, queues, streams); master captures it under the barrier with `SnapshotImage::capture`, streams and serialization outside it
- [x] 1.5 Replica: validate and decode the whole image first, then clear and load every covered store (clear then load); unknown versions fail with a clear error; the 1.3.x `FullSync` (KV + streams) is still accepted; a failed load forces a full sync on reconnect
- [x] 1.6 Sync decision: full sync when the replica asks for offset 0, for an offset older than the log or beyond the master's offset

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
