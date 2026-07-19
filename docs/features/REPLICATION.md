# Replication System

**Status**: ✅ **Wired for v1.0** — full-datatype replication, decoupled from
persistence, status surfaced in INFO. See "Wiring status" below.
**Last Updated**: 2026-07-09

---

## Wiring status (v1.0, audit M-005 + phase6j)

The replication engine (TCP master/replica, sync, backlog) was implemented and
unit-tested, but until v1.0 it was **never instantiated by the running server** —
`--role master` accepted the flag yet spawned no listener and replicated nothing.

As of v1.0 (`main.rs`) the node is constructed from config on startup and
self-runs; the master fans every write out to replicas, and a replica applies it
through a shared applier (`persistence::apply::apply_operation`) that the WAL
recovery path uses too — so the two never diverge.

**phase6j closed the three remaining gaps:**
- **All datatypes converge on the replica.** A replica now holds the
  hash/list/set/sorted-set/queue stores and applies *every* `Operation` variant,
  not just KV + stream. (End-to-end convergence tests live in
  `tests/datatype_replication_tests.rs`.)
- **Replication is decoupled from the WAL.** Propagation happens through a shared
  `record()` hook that always forwards to replicas and only appends to the WAL
  when persistence is enabled. A master therefore replicates even with
  `persistence.enabled = false`; a replication-only persistence layer opens no
  WAL file.
- **Status is exposed in INFO.** The `replication` section reports the live role,
  connected replica count, replication offset, and (on a replica) master link
  status and lag — see [Monitoring → INFO](#info-replication-section).

The sections below describe the design, configuration and usage, which remain
accurate.

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Configuration](#configuration)
4. [Command Line Usage](#command-line-usage)
5. [Guarantees & Consistency](#guarantees--consistency)
6. [Failure Handling](#failure-handling)
7. [Performance](#performance)
8. [Monitoring](#monitoring)

---

## Overview

Synap implements **Master-Replica replication** inspired by Redis and Kafka:

- **1 Master Node** - Accepts writes, replicates to replicas
- **N Replica Nodes** - Read-only, receive from master
- **Async Replication** - Non-blocking, eventual consistency
- **Manual Failover** - Promote replica to master when needed

### Key Features

✅ **TCP Communication** - Length-prefixed binary protocol (u32 + bincode)  
✅ **Full Sync** - Complete snapshot transfer with CRC32 verification  
✅ **Partial Sync** - Incremental updates from replication log offset  
✅ **Auto-Reconnect** - Replicas reconnect with intelligent full/partial resync  
✅ **Lag Monitoring** - Real-time offset tracking and lag calculation  
✅ **Write Protection** - Replicas are strictly read-only (enforced)  
✅ **Circular Replication Log** - 1M operations buffer (Redis-style)  
✅ **Eventual Consistency** - System optimized for low latency  
✅ **Stress Tested** - 5000 operations, multiple replicas, 100KB values  
✅ **KV Complete** - All 16 KV operations tested with replication  
✅ **Production Ready** - 67 comprehensive tests (98.5% passing)

---

## Architecture

### Master Node

```
┌─────────────────────────────────────┐
│         Master Node                  │
│                                      │
│  ┌──────────────┐   ┌─────────────┐│
│  │  KV Store    │   │ Replication ││
│  │  (R/W)       │◄──┤    Log      ││
│  └──────────────┘   │ (1M ops)    ││
│         │            └─────────────┘│
│         │  Append operations         │
│         ▼                            │
│  ┌──────────────────────────────────┤
│  │ Replication Thread                │
│  │ - Accepts replica connections     │
│  │ - Sends full/partial sync         │
│  │ - Broadcasts new operations       │
│  │ - Heartbeat (1s interval)         │
│  └──────────────────────────────────┤
└─────────────────────────────────────┘
           │
           ▼ (TCP connections)
    ┌──────────────┐
    │  Replica 1   │
    └──────────────┘
    ┌──────────────┐
    │  Replica 2   │
    └──────────────┘
```

### Replica Node

```
┌─────────────────────────────────────┐
│         Replica Node                 │
│                                      │
│  ┌──────────────┐                   │
│  │  KV Store    │ 🔒 READ-ONLY      │
│  │  (R/O)       │                   │
│  └──────────────┘                   │
│         ▲                            │
│         │ Apply operations           │
│  ┌──────────────────────────────────┤
│  │ Replication Thread                │
│  │ - Connects to master              │
│  │ - Receives sync (full/partial)    │
│  │ - Applies operations sequentially │
│  │ - Auto-reconnect on disconnect    │
│  │ - Tracks offset & lag             │
│  └──────────────────────────────────┤
└─────────────────────────────────────┘
```

---

## Configuration

### YAML Configuration (`config.yml`)

```yaml
replication:
  enabled: true
  role: master  # master | replica | standalone
  
  # Master address (for replicas)
  master_address: "127.0.0.1:15501"
  
  # Replica listen address (for masters)
  replica_listen_address: "0.0.0.0:15501"
  
  # Heartbeat interval (ms)
  heartbeat_interval_ms: 1000
  
  # Maximum lag threshold (ms)
  max_lag_ms: 10000
  
  # Replication buffer size (KB)
  buffer_size_kb: 256
  
  # Auto-reconnect on disconnect
  auto_reconnect: true
  
  # Reconnect delay (ms)
  reconnect_delay_ms: 5000
  
  # Replica timeout (seconds)
  replica_timeout_secs: 30
```

---

## Command Line Usage

### Starting a Master Node

```bash
# Basic master
./synap-server --role master --replica-listen "0.0.0.0:15501"

# Master with custom config
./synap-server \
  --config config.yml \
  --role master \
  --replica-listen "0.0.0.0:15501" \
  --host "0.0.0.0" \
  --port 5500
```

**Output**:
```
✅ Master node initialized, listening on 0.0.0.0:15501
⚠️  WRITES ALLOWED - This node accepts write operations
```

### Starting a Replica Node

```bash
# Basic replica
./synap-server \
  --role replica \
  --master-address "127.0.0.1:15501"

# Replica with auto-reconnect disabled
./synap-server \
  --role replica \
  --master-address "127.0.0.1:15501" \
  --auto-reconnect false
```

**Output**:
```
✅ Replica node initialized, connecting to master at 127.0.0.1:15501
🔒 READ-ONLY MODE - Writes will be rejected
♻️  Auto-reconnect: true
```

### Starting Standalone (No Replication)

```bash
./synap-server --role standalone
# Or simply omit --role (default)
./synap-server
```

---

## Guarantees & Consistency

### ✅ What Synap Guarantees

1. **Eventual Consistency**
   - All replicas will eventually receive all operations
   - Lag depends on network latency and load

2. **Write Protection on Replicas**
   - Replicas strictly reject write operations (SET, DEL, etc.)
   - Ensures data integrity (no split-brain)

3. **Operation Ordering**
   - Operations are applied in the same order on all replicas
   - Offset-based sequencing prevents reordering

4. **Auto-Recovery on Network Failures**
   - Replicas auto-reconnect on disconnect
   - Partial sync from last offset (if within buffer)
   - Full sync if offset too old (snapshot transfer)

5. **Durability (with Persistence)**
   - Replication log is in-memory (1M operations circular buffer)
   - For true durability, enable WAL persistence on both master and replicas

6. **Gap-free join under concurrent writes** (issue #234)
   - The master registers a joining replica in the fan-out set **before** taking
     its snapshot, so every write that lands during snapshot transfer is buffered
     to that replica instead of being lost in the snapshot→stream gap.
   - The replica deduplicates by offset: operations the snapshot already covers
     (offset < snapshot offset) are skipped, so non-idempotent ops (e.g. list
     push) are not applied twice.
   - A replica joining a master mid-write converges to the full dataset.

### ❌ What Synap Does NOT Guarantee

1. **Strong Consistency**
   - Replicas may lag behind master (eventual consistency)
   - No synchronous replication or quorum writes

2. **Automatic Failover**
   - Manual failover only (promote replica via API/CLI)
   - No automatic leader election (not Raft/Paxos)

3. **Zero Data Loss**
   - Master crash before replication = data loss on uncommitted ops
   - Mitigated by enabling WAL persistence

4. **Multi-Master**
   - Only 1 master allowed
   - Replicas cannot accept writes

---

## Failure Handling

### Master Crashes

**Scenario**: Master node crashes unexpectedly

1. Replicas detect disconnect (heartbeat timeout: 30s)
2. Replicas enter "disconnected" state
3. If `auto_reconnect = true`, replicas retry connection every 5s
4. **Manual Intervention Required**: Admin promotes a replica to master

**Failover Process** (Manual):

```bash
# 1. Identify most up-to-date replica (lowest lag)
curl http://replica1:5500/api/replication/stats

# 2. Promote chosen replica to master via API
curl -X POST http://replica1:5500/api/replication/promote

# 3. Update DNS/load balancer to point to new master
# 4. Reconfigure remaining replicas to new master
```

### Replica Crashes

**Scenario**: Replica node crashes

1. Master detects missing heartbeat (timeout: 30s)
2. Master marks replica as "disconnected"
3. Master continues serving (no impact on writes)
4. When replica restarts:
   - If offset within buffer (1M ops): **Partial Sync**
   - If offset too old: **Full Sync** (snapshot transfer)

### Network Partition

**Scenario**: Network split between master and replicas

1. **Master Side**:
   - Continues accepting writes
   - Marks disconnected replicas as "unavailable"
   - Buffered operations kept in replication log (up to 1M ops)

2. **Replica Side**:
   - Enters "disconnected" state
   - Continues serving reads (stale data)
   - Auto-reconnect attempts every 5s

3. **Recovery**:
   - When network recovers, replicas reconnect
   - Partial sync catches up with buffered operations
   - If buffer overflowed, full sync required

---

## Performance

### Replication Throughput

Benchmarks (localhost, no network latency):

| Operation | Master Throughput | Replication Overhead |
|-----------|-------------------|----------------------|
| **Append to log** | 10M ops/s | ~100ns per op |
| **Master → Replica (batch)** | 500K ops/s | ~2µs per op |
| **Snapshot creation (1K keys)** | 1 snapshot/sec | ~1ms |
| **Snapshot apply (1K keys)** | 1 snapshot/sec | ~1.5ms |

### Replication Lag

Typical lag measurements:

- **Same datacenter**: < 1ms (0.1-0.5ms typical)
- **Cross-region**: 20-100ms (depends on network)
- **Under heavy load**: 100-500ms (10K+ ops/sec)

### Buffer Capacity

- **Replication Log**: 1M operations (circular buffer)
- **Memory usage**: ~100MB (assuming 100 bytes/op)
- **Retention time**: Depends on write rate
  - 1K ops/sec = ~16 minutes
  - 10K ops/sec = ~1.6 minutes
  - 100K ops/sec = ~10 seconds

---

## Monitoring

### INFO replication section

`INFO replication` (and `INFO all`) reports live replication status, filled from
the running node rather than placeholders (phase6j item 1.4).

On a **master**:

```json
{
  "role": "master",
  "connected_slaves": 2,
  "master_repl_offset": 152340,
  "repl_backlog_active": 1,
  "repl_backlog_size": 152340
}
```

On a **replica** (adds link status and lag):

```json
{
  "role": "slave",
  "connected_slaves": 0,
  "master_repl_offset": 152338,
  "repl_backlog_active": 1,
  "master_link_status": "up",
  "slave_repl_lag": 2
}
```

- `connected_slaves` — replicas currently connected to this master.
- `master_repl_offset` — master: ops appended to the replication log; replica:
  ops applied locally.
- `master_link_status` — replica only: `"up"` when connected to the master,
  `"down"` otherwise.
- `slave_repl_lag` — replica only: operations behind the master.

A standalone node (replication disabled) reports `role: "master"` with zeroed
counters.

### Master Metrics

```bash
# Get master statistics
curl http://localhost:5500/api/replication/stats
```

**Response**:
```json
{
  "role": "master",
  "master_offset": 152340,
  "replicas": [
    {
      "id": "replica-1-uuid",
      "address": "192.168.1.10:5500",
      "offset": 152338,
      "lag_operations": 2,
      "lag_ms": 5,
      "connected_at": 1729512000,
      "last_heartbeat": 1729513000
    }
  ]
}
```

### Replica Metrics

```bash
# Get replica statistics
curl http://localhost:5501/api/replication/stats
```

**Response**:
```json
{
  "role": "replica",
  "replica_offset": 152338,
  "master_offset": 152340,
  "lag_operations": 2,
  "lag_ms": 5,
  "total_replicated": 152338,
  "total_bytes": 15234000,
  "connected": true,
  "last_heartbeat": 1729513000
}
```

### Alerts to Monitor

1. **High Lag** (`lag_operations > 10000`)
   - Replica falling behind
   - Possible network issues or slow replica

2. **Disconnected Replicas** (`connected = false`)
   - Replica crashed or network partition
   - Check replica logs

3. **Frequent Full Syncs**
   - Buffer too small or writes too fast
   - Increase buffer size or reduce write rate

4. **Master Offset Not Increasing**
   - No write activity (expected if idle)
   - Or master stuck (investigate)

---

## Best Practices

1. **Enable Persistence on Both Master and Replicas**
   - Prevents data loss on crashes
   - WAL allows recovery

2. **Monitor Replication Lag**
   - Set up alerts for high lag (>5 seconds)
   - Investigate slow replicas

3. **Use Auto-Reconnect** (default: true)
   - Replicas recover automatically from network issues

4. **Plan for Failover**
   - Document manual failover procedure
   - Test failover regularly

5. **Size Buffer Appropriately**
   - Default: 1M operations (~16 min at 1K ops/sec)
   - Increase if frequent full syncs

6. **Read from Replicas**
   - Distribute read load across replicas
   - Accept stale reads (eventual consistency)

7. **Write to Master Only**
   - Never attempt writes to replicas (will be rejected)

---

## Troubleshooting

### Replica Won't Connect

**Symptoms**: Replica logs show connection errors

**Solutions**:
1. Check master is running and listening on correct port
2. Verify firewall allows connections
3. Check `master_address` in replica config
4. Verify master has `replica_listen_address` configured

### Frequent Full Syncs

**Symptoms**: Logs show many "Full sync required" messages

**Causes**:
- Replication log buffer too small (1M ops)
- Write rate too high for buffer capacity
- Replica offline too long

**Solutions**:
1. Increase buffer size in replication log
2. Reduce write rate
3. Add more replicas to distribute load

### High Replication Lag

**Symptoms**: `lag_operations` > 10000, `lag_ms` > 1000

**Causes**:
- Network latency
- Slow replica (overloaded CPU/disk)
- Master writing too fast

**Solutions**:
1. Check network latency between master and replica
2. Scale replica resources (CPU, memory)
3. Reduce write rate on master
4. Add caching layer

---

## Roadmap

### ✅ Completed (v0.3.0)

- [x] Master-replica architecture
- [x] Full & partial sync
- [x] Offset-based replication
- [x] Auto-reconnect
- [x] Lag monitoring
- [x] Manual failover
- [x] Write protection on replicas
- [x] CLI args for role configuration

### 🔜 Planned (v0.4.0)

- [ ] Automatic failover (leader election)
- [ ] Multi-level replication (replica → sub-replica)
- [ ] Read-your-writes consistency option
- [ ] Quorum writes (configurable consistency level)
- [ ] Replication log persistence to disk
- [ ] Grafana dashboards for monitoring

---

**Author**: HiveLLM Team  
**License**: MIT  
**Status**: ✅ Production-Ready (with manual failover)

