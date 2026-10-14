# Synap Migration Tool

Migrates a standalone Synap data directory into one tenant's namespace for
HiveHub.Cloud (Hub) mode.

The tool reads the data directory the way the server recovers it at startup,
renames every resource to the name Hub mode gives it for the tenant
(`user_{user_id}:{name}`), and writes the result to a **new** directory as one
snapshot in the server's current format. The original directory is only read,
so it stays your rollback.

## Installation

```bash
cargo build --release --bin synap-migrate
```

The binary is `target/release/synap-migrate` (`synap-migrate.exe` on Windows).

## Commands

### migrate

```bash
synap-migrate migrate \
  --data-dir ./data \
  --output ./data-hub \
  --user-id 550e8400-e29b-41d4-a716-446655440000
```

| Option | Meaning |
|---|---|
| `--data-dir` | The standalone data directory: `snapshots/` and `wal/synap.wal`, the server's default layout (default `./data`). It is not modified. |
| `--output` | Where to write the migrated data directory. It must not exist yet or be an empty directory, and must not be inside `--data-dir`. Required. |
| `--user-id` | The Hub user ID (tenant) that owns every migrated resource. |
| `--server-config` | The configuration file of the server that wrote the data. Its `queue` settings configure the queues the data records no configuration for (snapshots older than v5, queues created implicitly by an old publish record). Without it they get the server's built-in queue defaults. |
| `--dry-run` | Verify the input and print the report without writing anything. |

What it does, in order:

1. Checks the output location.
2. Verifies the whole input before writing anything: **every** snapshot file
   must load and pass its checksum (startup may fall back past a damaged
   newest file; the migration does not), and the whole WAL must scan. Corrupt
   or unsupported input fails the run with an error that names the file and
   the reason, and nothing is written.
3. Rebuilds the state as the server would recover it: the newest snapshot plus
   every WAL record written after it, replayed with the server's own code.
4. Refuses input that already holds Hub-scoped names (it was migrated before).
5. Renames every resource for the tenant and writes
   `<output>/snapshots/snapshot-v5-00000000000000000001.bin`.
6. Loads the written snapshot back with the server's loader and checks it
   holds what was written.

It prints what it read, structure by structure:

```
Snapshot: ./data/snapshots/snapshot-v5-00000000000000000007.bin (format v5)
WAL: ./data/wal/synap.wal (format v2): 49 records, 19 replayed after the snapshot
Read:
  KV keys:       5 (3 with expiry)
  hashes:        1
  lists:         1
  sets:          1
  sorted sets:   2 (geospatial included)
  HyperLogLogs:  1
  bitmaps:       1
  queues:        3 (5 messages, 1 dead letters)
  streams:       1 (3 events)
Wrote: ./data-hub/snapshots/snapshot-v5-00000000000000000001.bin
```

`Note:` lines tell you what the input format could not carry (see
[Older data directories](#older-data-directories)) and report a torn WAL tail.

### validate

```bash
synap-migrate validate --data-dir ./data-hub --user-id 550e8400-e29b-41d4-a716-446655440000
```

Checks that every resource of every structure in the directory belongs to
the tenant.

### status

```bash
synap-migrate status --data-dir ./data
```

Reports `NOT MIGRATED`, `MIGRATED` (with the tenants found) or `PARTIALLY
MIGRATED` (with an example of an unscoped resource).

## Migration workflow

1. **Stop the server** cleanly (Ctrl-C or SIGTERM), so nothing writes to the
   directory during the migration.
2. **Dry run**: `synap-migrate migrate --data-dir ./data --output ./data-hub --user-id $USER_ID --dry-run`.
   Read the report and its notes.
3. **Migrate** into a new directory with the same command without `--dry-run`.
4. **Point the Hub-mode server at the output**: in its configuration set
   `persistence.snapshot.directory` to `./data-hub/snapshots` and
   `persistence.wal.path` to `./data-hub/wal/synap.wal`, and enable `hub`.
   The output has no WAL yet; the server creates it at its first start.
5. **Rollback** if needed: stop the server, point it back at the original
   directory with Hub disabled, and delete the output directory. The original
   directory was never written to.

## What gets migrated

Every structure the server persists, renamed with the same functions the Hub
handlers use (`synap_server::hub::MultiTenant`):

| Structure | Hub name | Kept |
|---|---|---|
| KV keys | `scope_kv_key` | value, absolute expiry deadline |
| hashes, lists, sets | `scope_kv_key` | every field, element and member |
| sorted sets and geospatial indexes | `scope_kv_key` | every member and score |
| HyperLogLogs, bitmaps | `scope_kv_key` | registers and bits, expiry |
| queues | `scope_queue_name` | configuration; every message with its ID, payload, priority, retry counters, creation time and headers; dead letters |
| streams | `scope_stream_name` | every event of the snapshot's stream section with its ID, offset, type, data and timestamp |

What any restart changes, the migration changes the same way: keys whose
deadline has passed are left out, and messages delivered but never
acknowledged are ready again (with their ID and retry count).

Streams are not in the WAL: they are durable only through snapshots, and the
server does not restore a snapshot's stream section at startup (in this or
earlier releases). The migration carries the stream section into the output
snapshot unchanged except for the room names.

## Formats read

Everything the server reads, through the server's own code:

- snapshots in formats v2 to v5, under sequence names
  (`snapshot-v<format>-<20-digit sequence>.bin`, 1.4.0 and later) and legacy
  names (`snapshot-v<format>-<unix seconds>.bin`); the newest one is used;
- WAL files with the `SYNAPWAL` version 2 header and headerless 1.3.x files.
  A torn tail (bytes a crash left mid-append) is skipped and reported, as
  startup discards it; unlike startup, the migration does not truncate it.
  A complete record whose checksum matches but which this release cannot
  decode (a record type a newer release added) is not a torn tail: the run
  fails instead of dropping it.

### Older data directories

A directory written by Synap 1.3.x holds less than a current one, so no
server, and no migration, gets the rest back:

- a v2 or v3 snapshot stores no key expiry: the keys it holds load without
  expiry, as the server loads them (keys written to the WAL after it keep
  their deadline);
- snapshots older than v5 store no queue configuration and no in-flight or
  dead-lettered messages; use `--server-config` to give those queues the
  source server's queue settings;
- 1.3.x did not persist HyperLogLogs and bitmaps, and did not log hash, list,
  set and sorted-set writes to the WAL.

The report prints a `Note:` for each of these that applies.

## Design: one persistence codec

The migrator has no snapshot or WAL code of its own. It depends on the
`synap-server` library and uses its persistence module:
`SnapshotManager` lists, loads and writes snapshot files,
`wal_frame::scan` reads WAL files, `recovery::rebuild` replays the records
onto the snapshot exactly as startup recovery does, and `SnapshotImage` is the
in-memory form both the server's snapshots and the migration write.

The alternative was moving the codecs into `synap-core`. They are not
separable from the server: the WAL record set (`Operation`), its bincode
framing, the snapshot writer (which captures the stores through `StoreRefs`)
and the replay applier all live in `synap-server`'s persistence module, so the
move would have carried most of that module into the engine crate.
`synap-server` does not depend on `synap-migrate`, so the dependency compiles
without a cycle and keeps one implementation of each format. The cost is a
longer build of an offline tool that links the server library.

## Testing

```bash
cargo test -p synap-migrate --lib
cargo test -p synap-migrate --features full-tests --test real_server_fixtures_tests
```

The unit tests cover the WAL formats and tail, the scoping of every
structure, and the rejection of corrupt, unsupported and misplaced input.

`real_server_fixtures_tests` migrates data directories written by a real
1.3.2 server and by this release's server (`tests/fixtures/`, see its README
and `generate_fixtures.py`), then recovers the output in Hub mode in-process:
a Hub-enabled `ServerConfig` recovered by `persistence::recover_all` as
`synap-server` starts, served by the production router with a `HubClient`.
The binary itself cannot serve those reads, because Hub request
authentication refuses every request until the HiveHub SDK can validate
access keys (hivellm/synap#230); the test's `HubClient` maps one access key to
the tenant instead. The tenant then reads every resource and the test
compares it with what the source server served before it stopped.

## Troubleshooting

| Error | Meaning |
|---|---|
| `the output ... is not empty` | Pick a new directory, or empty that one. |
| `the output ... is inside the data directory` | The source must stay unchanged; write elsewhere. |
| `the input already holds Hub-scoped names` | The directory was migrated before (`synap-migrate status` shows it). |
| `Snapshot "..." is damaged: checksum mismatch ...` | That file is corrupt. Restore it from a backup, or move it out of `snapshots/` if you accept losing what only it holds. |
| `Snapshot "..." is damaged: unknown magic ...` | Not a snapshot, or one written by a newer Synap: use that release's migrator. |
| `WAL ... is corrupted at byte N ... interior corruption` | A damaged record is followed by valid ones; restore the WAL from a backup. |
| `WAL ... uses format version N` | Written by a newer Synap: use that release's migrator. |
| `the WAL ... holds a complete record at byte N that this release cannot read` | The WAL ends with records of a type this release does not know, written by a newer Synap: use that release's migrator. |

## Resource naming

After migration every resource is named `user_{user_id}:{name}`, the user ID
in its 32-digit form. For user `550e8400-e29b-41d4-a716-446655440000`, queue
`my-queue` becomes `user_550e8400e29b41d4a716446655440000:my-queue` and KV
key `config:app` becomes `user_550e8400e29b41d4a716446655440000:config:app`.

## See also

- [Migration guide](../../docs/guides/MIGRATION_GUIDE.md)
- [HiveHub configuration guide](../../docs/guides/HUB_CONFIGURATION.md)
- [Hub integration specification](../../docs/specs/HUB_INTEGRATION.md)
