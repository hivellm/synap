# Replica Full Sync

## ADDED Requirements

### Requirement: Full sync carries every replicated datatype
A replica that receives a full sync MUST end with every KV key (with its
absolute expiry), hash, list, set, sorted set, HyperLogLog, bitmap, queue and
stream the master held when the image was taken.

#### Scenario: Replica connects after the master holds data
Given a master holding a KV key with a deadline, a hash, a list, a set, a sorted set, a geo set, a HyperLogLog, a bitmap, a queue message and a stream event
When a new replica connects
Then the replica holds every one of them with the master's values
And the KV key keeps the master's absolute deadline

### Requirement: Full-sync image paired with its replication offset
The master SHALL take the full-sync image under the write barrier, so every
replicated write is either in the image (offset below the image offset) or
streamed after it, and the replica MUST apply every write exactly once.

#### Scenario: Writes during the full sync
Given a full sync paused while it holds the write barrier
When clients run LPUSH, HINCRBY and INCR on keys present in the image
And the full sync then completes
Then those writes wait until the barrier is released
And the replica's lists, hash counters and KV counters equal the master's

#### Scenario: Concurrent writers while a replica connects
Given 8 clients running LPUSH, HINCRBY and INCR on distinct keys continuously
When a replica connects and completes its full sync while they run
Then after the writers stop the replica's data equals the master's

### Requirement: Replicas receive records in offset order
The master MUST queue each replicated record for fan-out in replication
offset order, so a replica never receives a lower offset after a higher one.

#### Scenario: Concurrent writers to different keys
Given 8 tasks replicating records concurrently
When a replica reads them from its connection
Then the offsets it receives are strictly increasing

### Requirement: Replica replaces its state atomically
A replica MUST validate and decode the whole full-sync image before changing
any store, and then SHALL replace (clear, then load) every store the image
covers before applying any streamed command.

#### Scenario: Resync after disconnect drops stale keys
Given a replica that holds keys the master deleted while it was disconnected
And the master's replication log no longer covers the replica's offset
When the replica reconnects and receives a full sync
Then the deleted keys are gone from the replica
And every key the master holds is present

#### Scenario: Corrupt image leaves the replica untouched
Given a replica holding data
When it receives a full-sync image whose checksum does not match
Then the sync fails with a checksum error
And the replica's data is unchanged

### Requirement: Versioned full-sync payload
The full-sync payload MUST carry a format version. A replica MUST refuse an
image version it does not support with an error naming the received and the
supported versions, and SHALL keep accepting the 1.3.x KV + stream image so
replicas can be upgraded before the master.

#### Scenario: Newer image version
Given a replica that supports full-sync version 2
When it receives a full-sync image of version 3
Then the sync fails with an error naming version 3 and version 2
And the replica's data is unchanged

#### Scenario: 1.3.x master
Given an upgraded replica
When a 1.3.x master sends its KV + stream full sync
Then the replica loads the KV keys and stream events

### Requirement: Full sync for replicas without usable history
The master MUST send a full sync when the replica asks for offset 0, for an
offset older than its replication log, or for an offset beyond its current
offset.

#### Scenario: Fresh replica after the master already wrote
Given a master that recovered data at boot and has replicated writes since
When a fresh replica connects asking for offset 0
Then it receives a full sync containing the recovered data
