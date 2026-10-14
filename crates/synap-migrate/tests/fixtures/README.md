# synap-migrate fixtures

Data directories written by real Synap servers, used by
`tests/real_server_fixtures_tests.rs`; `v1.3.2` is also the directory that
synap-server's `tests/upgrade_from_1_3_2_tests.rs` starts the server binary on
(the in-place upgrade to this release). Nothing here is written by the code
under test: `generate_fixtures.py` starts a real server binary with
persistence on, drives every datatype through `/api/v1/command`, forces one
snapshot (`POST /snapshot`), writes more data that only the WAL holds, records
what the server serves for every resource (`baseline.json`), stops the server
with Ctrl-C and copies its data directory here.

Each fixture holds:

- `data/snapshots/` and `data/wal/synap.wal`: the server's data directory,
  byte for byte (see `.gitattributes`).
- `baseline.json`: what the source server served right before it stopped
  (`captured_at_ms`), plus the message IDs the script published, acknowledged
  and left in flight.

## Dataset

Before the snapshot:

| Structure | Writes |
|---|---|
| KV | `plain` = `value-a`; `ttl-long` = `lives` (TTL 10 years); `deleted` set then deleted |
| hash | `user:1` fields `name`, `email`, `scratch`; `scratch` deleted |
| list | `events` = a, b, c, then one LPOP |
| set | `tags` = x, y, z, then `y` removed |
| sorted set | `scores` = alice 10, bob 20.5, carol 5 |
| geospatial | `places` = sao-paulo, paris (a sorted set of geohash scores) |
| HyperLogLog | `visitors` = u1..u5 |
| bitmap | `flags` bits 1, 7, 100 |
| queue `jobs` | job-1..job-4 (priorities 5, 9, 5, 1), ACK deadline 1 h; job-2 delivered and acked, job-1 delivered and left in flight, job-3 delivered and NACKed with requeue (retry count 1) |
| queue `dead` | `poison` published with `max_retries` 0, delivered and NACKed: dead-lettered |
| stream | room `feed`: three `created` events (offsets 0 to 2) |

After the snapshot (the WAL tail):

| Structure | Writes |
|---|---|
| KV | `plain` = `value-b`; `tail-plain` = `after`; `ttl-tail` (TTL 10 years); `ttl-short` (TTL 60 s, expired long before any test runs); `tail-deleted` set then deleted |
| hash / list / set / sorted set | `user:1.city` = London; `events` += d; `tags` += w; `scores` += dave 1 |
| HyperLogLog / bitmap | `visitors` += u6; `flags` bit 200 |
| queue `jobs` | job-1 acked (it was in flight at the snapshot); job-5 (5) and job-6 (7) published; job-6 delivered and left in flight |
| queues `temp`, `late` | `temp` created and deleted; `late` created with `max_depth` 500 and `late-1` published |
| stream | `feed`: one `after-snapshot` event (offset 3) |

## v1.3.2

Synap 1.3.2: the release binary of the detached checkout at `bfecd0c`.

- `snapshot-v3-<unix seconds>.bin`: format v3 under the legacy name, WAL
  cutoff 13. KV keys without expiry; queues as their ready messages only, in
  the message's API encoding (no creation time, no configuration, no in-flight
  or dead-lettered messages); streams; hashes, lists, sets, sorted sets. No
  HyperLogLog or bitmap section.
- `wal/synap.wal`: headerless WAL (format 1), records 1 to 22. Records 13 to
  22 are the tail: `KVSet` (relative TTLs) and `KVDel`, `QueueAck`,
  `QueuePublish`.

What 1.3.2 does not persist, so no server (and no migration) gets it back
after a restart, although `baseline.json` shows the live server serving it:

- the expiry of keys the snapshot holds (`ttl-long` loads without expiry);
- HyperLogLog and bitmap values (never logged, no snapshot section);
- hash, list, set and sorted-set writes after the snapshot (1.3.2 did not log
  them to the WAL), so `user:1.city`, `d`, `w` and `dave` are gone;
- queue configuration (`late` comes back with the default `max_depth`), queue
  creation and deletion, dead letters and in-flight messages;
- the stream event published after the snapshot (streams live in snapshots
  only, in every release).

## v1.4.0

The 1.4.0 release line (`release/v1.4.0` with phase25, generated at
`2e27e48`), debug build. Its crate version still reads 1.3.2 until the
release bump, so `/health` reports `1.3.2`.

- `snapshot-v5-00000000000000000001.bin`: format v5 under a sequence name,
  WAL cutoff 31. KV keys with absolute expiry; whole queues (configuration,
  ready, in-flight and dead-lettered messages with IDs, priority, retry
  counters, creation time); streams; every collection; HyperLogLogs; bitmaps.
- `wal/synap.wal`: `SYNAPWAL` version 2, records 1 to 49. Records 31 to 49
  are the tail: `KVSet`, `KVSetAt`, `KVDel`, `HashSet`, `ListPush`, `SetAdd`,
  `ZAdd`, `HllAdd`, `BitmapSetBit`, `QueueAck`, `QueuePublishFull`,
  `QueueCreate`, `QueueDelete`.

What 1.4.0 does not persist: the stream event published after the snapshot,
and deliveries (a message in flight at the stop is ready again after a
restart, with its ID and retry count).

## Regenerating

From the repository root, with PyYAML installed:

```sh
python crates/synap-migrate/tests/fixtures/generate_fixtures.py \
    --label v1.3.2 --server <1.3.2 checkout>/target/release/synap-server \
    --base-config <1.3.2 checkout>/config/config.yml
cargo build -p synap-server
python crates/synap-migrate/tests/fixtures/generate_fixtures.py \
    --label v1.4.0 --server target/debug/synap-server --base-config config/config.yml
```

IDs and timestamps change with every run; update the cutoffs and record
numbers above if the dataset changes.
