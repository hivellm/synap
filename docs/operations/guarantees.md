# Synap 1.4.0 guarantees

What a Synap 1.4.0 server promises about your data and your access rules, and
where those promises stop. Every statement on this page was checked against the
1.4.0 source; each section ends with the code it describes. Paths are relative
to `crates/synap-server/src/` unless they name another crate.

Older documents, including the historical analyses under `docs/analysis/`,
describe earlier revisions. Where they disagree with this page, this page
describes 1.4.0.

- [1. Durability](#1-durability)
- [2. What an error reply means for in-memory state](#2-what-an-error-reply-means-for-in-memory-state)
- [3. Authorization per transport](#3-authorization-per-transport)
- [4. Hub mode](#4-hub-mode)
- [5. Limits](#5-limits)
- [6. Replication](#6-replication)
- [7. File formats and upgrades](#7-file-formats-and-upgrades)

---

## 1. Durability

### 1.1 What is persisted at all

| Data | WAL | Snapshots | Replicated | Survives a restart |
|---|---|---|---|---|
| Strings (with expiry), hashes, lists, sets, sorted sets and geospatial keys, HyperLogLog, bitmaps | yes | yes | yes | yes |
| Queues: messages, configuration, ACK/NACK, purge, delete | yes | yes (each queue whole) | yes | yes, see [1.8](#18-transactions-scripts-and-queues) |
| Event streams (`stream.*`) | no | written, never loaded | only inside a full sync | **no** |
| Partitioned topics and consumer groups | no | no | no | **no** |
| Pub/Sub messages, KV watch and keyspace notifications | no | no | no | delivered live only |
| Users, their roles, API keys | no | no | no | **no**; the root user is re-created from `auth.root` at startup when its password is valid (see [3.1](#31-modes)) |
| Lua script cache (`SCRIPT LOAD`) | no | no | no | **no** |

Event streams deserve a plain warning: a snapshot file contains the stream
events, but startup never reads that section and builds an empty stream
manager, and live stream publishes are neither logged nor sent to replicas.

With `persistence.enabled: false` there is no WAL and no snapshot: every write
is acknowledged once it is applied in memory (a replication master still
forwards it to replicas). With `persistence.wal.enabled: false` the WAL is off
but snapshots are still taken, so a crash loses everything written since the
last snapshot.

Code: `persistence/layer.rs:532-548` (stream publish is a no-op for the WAL),
`persistence/recovery.rs:135-247` (snapshot restore reads no stream section),
`main.rs:492-516` (fresh stream, partition and consumer-group managers),
`replication/master.rs:331-340` (stream events only in a full sync),
`auth/user.rs:133-145` and `auth/api_key.rs:179-185` (in-memory maps),
`main.rs:793-823` (root user from config), `persistence/layer.rs:106-116`
(WAL opened only when `persistence.enabled` and `wal.enabled`),
`persistence/layer.rs:550-570` (snapshots need only `snapshot.enabled`).

### 1.2 When a write is acknowledged

Every write, on every transport, goes through one shared path:

1. The write locks its keys' ordering stripes.
2. It is applied in memory.
3. Its effect is handed to replication (when this node is a master) and queued
   for the WAL writer, still under the stripes.
4. The stripes are released and the caller waits for the WAL group that holds
   the record.
5. The reply is sent.

Consequences you can rely on:

- Writes to one key reach the WAL and replicas in the order memory applied
  them.
- A command that changes several keys or fields at once (MSET, HMSET, GEOADD
  and similar) is one WAL record.
- Derived writes are recorded as their result: INCR, APPEND, SETRANGE,
  HINCRBY, ZINCRBY record the value the key ends up with; pops record the
  members removed.
- A refused conditional write (NX/XX, an SREM that removed nothing) records
  nothing.

Consequences that are not guarantees:

- Another client can read a write after step 2, before it is durable and before
  its writer got a reply.
- Replication (step 3) happens before the WAL write, so a write that then fails
  to reach the WAL has already been handed to replicas (see
  [section 2](#2-what-an-error-reply-means-for-in-memory-state)).

The writer handles up to 1000 queued records per group (group commit).

Code: `persistence/durable.rs:1-31` (the shared path and its rules),
`persistence/durable.rs:115-131` (SET as an example), `persistence/layer.rs:59-93`
(queue under the stripes, then wait), `persistence/layer.rs:196-209`
(replicate, then queue for the WAL), `persistence/layer.rs:245-251` (one
record per multi-effect command), `persistence/wal_async.rs:368-389` (group of
up to 1000).

### 1.3 Per fsync mode

`persistence.wal.fsync_mode`; the built-in default is `periodic` with
`fsync_interval_ms: 1000`. The shipped `config/config.yml` sets `always`.

| Mode | Done before the reply | Process killed or aborted | OS crash or power loss |
|---|---|---|---|
| `always` | written, handed to the OS and fsynced | no acknowledged write lost | no acknowledged write lost, provided the storage honors fsync |
| `periodic` | written and handed to the OS; fsync runs on its own timer every `fsync_interval_ms`, also when the server is idle | no acknowledged write lost | up to about `fsync_interval_ms` of acknowledged writes |
| `never` | written and handed to the OS; the writer never fsyncs on its own | no acknowledged write lost | any acknowledged write the OS had not written back yet |

Whatever the mode, the WAL is also fsynced before a snapshot is published and
during graceful shutdown. A write that was applied in memory but
had not been acknowledged when the process died may or may not be in the WAL.

A crash in the middle of a record leaves a torn last frame; the next startup
truncates it and appends after the last whole record.

Code: `persistence/types.rs:630-641` (defaults), `persistence/types.rs:667-677`
(modes), `persistence/wal_async.rs:116-120` (contract),
`persistence/wal_async.rs:372-398` (periodic timer),
`persistence/wal_async.rs:452-457` (each group handed to the OS before its
replies), `persistence/wal_async.rs:504-528` (when fsync runs),
`persistence/layer.rs:615-623` (fsync before a snapshot),
`persistence/wal_frame.rs:25-33` (torn tail).

### 1.4 Graceful shutdown

On SIGINT, or SIGTERM on Unix, the HTTP server stops accepting connections and
waits up to 5 seconds for in-flight HTTP requests. Then the WAL is flushed and
fsynced in every fsync mode and its writer stops; a write that arrives after
that point fails instead of being acknowledged. The RESP3 and SynapRPC
listeners are not stopped before the WAL, so a write arriving there after the
WAL stopped gets that error. On Windows only Ctrl-C triggers this path.

Code: `main.rs:1001-1051`, `main.rs:1054-1082`,
`persistence/wal_async.rs:295-302`, `persistence/wal_async.rs:438-444`.

### 1.5 WAL failures (poisoning)

A failed write, flush or fsync of the WAL fails every reply in its group and
poisons the writer until the process restarts:

- Every later write is still applied in memory, and still handed to
  replication on a master, and then answers the not-durable error described in
  [section 2](#2-what-an-error-reply-means-for-in-memory-state).
- `GET /health` answers `503` with `"status": "unhealthy"`, `"wal": "poisoned"`
  and the reason in `"wal_error"`, so a supervisor can restart the process.
- Restarting is the only way out; startup rescans and repairs the file.

Code: `persistence/wal_async.rs:207-213` (fail fast once poisoned),
`persistence/wal_async.rs:461-541` (write, flush and fsync failures poison),
`persistence/types.rs:46-52`, `server/handlers/mod.rs:227-264` (health).

### 1.6 Snapshots

- **When.** A background task checks once a minute and takes a snapshot when
  `snapshot.interval_secs` (default 300) have passed or
  `snapshot.operation_threshold` (default 10,000) records were logged since the
  last one. A failed snapshot is only logged; the next one is taken when a
  trigger fires again.
- **Consistent cut.** While a snapshot records its WAL cutoff and copies the
  stores, it holds every ordering stripe, so writers pause for the in-memory
  copy only. Each write is either in the snapshot or after its cutoff, never
  both and never neither. Serialization happens after writers resume.
- **WAL first.** The WAL is fsynced up to the cutoff before the snapshot file
  is published, in every fsync mode.
- **Crash-safe file.** The snapshot is written to a temporary file, fsynced,
  renamed to `snapshot-v<format>-<sequence>.bin`, and the directory is fsynced
  (Unix; on Windows the NTFS rename is relied on). A crash leaves either no new
  snapshot or the complete one; a leftover temporary file is ignored by
  recovery and removed by the next snapshot.
- **Retention.** Old snapshots are removed only after the new one is durable,
  never the one just written, and at least one is always kept
  (`max_snapshots: 0` keeps one). A failed removal is logged as a warning.
- **Recovery fallback.** Startup loads the newest snapshot that verifies. A
  damaged newer file is skipped only when the WAL still provably holds every
  record after the older snapshot's cutoff; otherwise startup refuses, naming
  the damaged files. A snapshot that cannot be read for another reason (for
  example permissions) also refuses startup, because it may be intact.

Back up the snapshot and WAL directories together: one is not complete without
the other (see [Backup](../users/operations/BACKUP.md)).

Code: `persistence/layer.rs:550-570` (trigger), `persistence/layer.rs:640-652`
(checked every 60 s), `persistence/layer.rs:573-636` (barrier, cutoff, WAL
fsync), `persistence/types.rs:654-665` (defaults),
`persistence/snapshot.rs:166-211` (temporary file, rename, directory fsync),
`persistence/snapshot.rs:323-338` (retention),
`persistence/wal_frame.rs:345-360` (directory fsync on Unix only),
`persistence/recovery.rs:64-73` and `persistence/recovery.rs:296-328`
(fallback rules).

### 1.7 Startup: what refuses to start and what starts empty

Startup **refuses** (the process exits with the error) when:

- the WAL has interior corruption (a bad frame followed by valid ones), naming
  the byte position;
- the WAL header is damaged or declares a newer format than this server reads;
- the newest snapshot is damaged and no older starting point is provably
  complete, or a snapshot cannot be read.

Startup **logs a warning and continues with empty stores** on any other
recovery error. That includes a WAL record that fails to apply during replay
(for example a queue ACK failing with something other than "not found") and an
I/O error opening the WAL. The unreplayed WAL is not deleted, but when the WAL
opens, new records are appended after the unreplayed ones and the next
snapshot's cutoff lies past them, so later restarts do not replay them either. Treat a "Recovery
failed ... starting fresh" warning as an incident and stop the server before
it takes a snapshot.

If the persistence layer itself cannot be opened for such a reason, the server
logs a warning and runs **without a WAL**: writes are acknowledged without
being logged, and `GET /health` reports `"wal": "disabled"`.

Expiry is stored as an absolute deadline (Unix milliseconds) in snapshots and
in the WAL. Keys whose deadline passed while the server was down are not
restored. Deadlines are compared with the wall clock, so changing the system
clock moves every expiry with it.

The WAL is never truncated after a snapshot and never rotated; startup scans
the whole file. `wal.max_size_mb` is not enforced. Plan disk space for a WAL
that grows with every write.

Code: `persistence/types.rs:73-91` (`refuses_startup`), `main.rs:399-436`
(refuse, else start empty), `persistence/types.rs:106-110` (a replay error
becomes `RecoveryFailed`, which does not refuse), `persistence/recovery.rs:254-273`
(replay stops at the first error), `main.rs:637-664` (no persistence layer on
other errors), `persistence/recovery.rs:143-171` and
`persistence/types.rs:510` (absolute expiry), `persistence/wal_frame.rs:244-256`
(the only truncation is a torn tail), `persistence/types.rs:620-628`
(`max_size_mb` is never read).

### 1.8 Transactions, scripts and queues

**MULTI/EXEC.** A committed transaction is one WAL record under one checksum,
so a crash at any byte recovers all of its effects or none. Replication ships
it as one command at one offset; a replica whose connection drops mid-record
applies none of it and receives it whole after reconnecting. A replica applies
the operations of that record one after another, so a reader on the replica can
see a transaction half-applied.

**Scripts.** `EVAL`/`EVALSHA` record each `redis.call` write on its own. A
script is not one record: a crash during a script can recover the first part of
its writes, and a script that fails part way keeps the writes it already made.

**Queues.** Each message keeps its ID, priority, retry counters, creation time
and headers through WAL replay, snapshots and replication. Queue create (with
its configuration), delete and purge are recorded. Deliveries are not recorded:
after a restart, a message that was delivered but not yet acknowledged is ready
again and is delivered again, and an ACK sent after the restart for that
earlier delivery answers "not found". Delivery is at-least-once. Requeues
caused by an expired ACK deadline are checked once a second and recorded as
NACKs, so retry counts and dead-lettering match after a restart.

Code: `persistence/wal_async.rs:249-264` and `persistence/layer.rs:253-269`
(one transaction record), `persistence/durable.rs:1637-1667` (EXEC on every
transport), `replication/replica.rs:300-320` (whole commands only),
`persistence/apply.rs:141-164` (operations applied in order),
`scripting/mod.rs:221-262` (each call authorized and recorded),
`persistence/apply.rs:20-23` (deliveries not logged),
`synap-core/src/core/queue/mod.rs:415-423` (ACK settles in-flight messages
only), `synap-core/src/core/queue/manager.rs:235-243` (restored in-flight
messages become ready), `persistence/durable.rs:1450-1615` (queue records),
`persistence/durable.rs:1544-1560` (deadline requeues).

---

## 2. What an error reply means for in-memory state

An error reply does **not** always mean nothing happened. The cases:

| Reply | In memory | In the WAL | On replicas |
|---|---|---|---|
| Not-durable error on a write: "write applied in memory but could not be logged to the WAL, so it is not durable and is not rolled back" | applied, visible to readers | no; lost on this node at restart | already handed to replication (master) |
| Not-durable error on EXEC: "transaction applied in memory but could not be logged to the WAL, so the commit is not durable and the effects are not rolled back" | every effect applied | no | already handed to replication (master) |
| EXEC fails on one of its queued commands (for example INCR on a value that is not an integer); the reply is that command's error | the commands before it are applied; nothing after it | **nothing** from this transaction | **nothing** from this transaction |
| `EXECABORT` (the transaction was aborted earlier, for example by a refused command) | nothing | nothing | nothing |
| A script error part way through | earlier `redis.call` writes stay applied | those writes are recorded | those writes are replicated |
| Authorization denial (HTTP 403, RESP3 `-NOPERM`, `NOPERM` text elsewhere) | nothing; checked before the command runs | nothing | nothing |
| Rate limit (429), body too large (413), Hub 401/403/503 | nothing; refused before any handler | nothing | nothing |

The third row is a real divergence: memory holds the partial transaction, while
the WAL and the replicas hold none of it. After a restart, or on a replica, the
partial effects are gone.

How each transport reports these errors:

- **REST**: the HTTP status of the error; a not-durable error is `500`.
- **`/api/v1/command`**: HTTP `200` with `"success": false` and the message in
  `"error"`, for command errors and authorization denials alike.
- **RESP3**: an error reply (`-NOPERM` for denials).
- **SynapRPC**: an error response carrying the message.
- **MCP and UMICP**: a tool error.
- **Queue WebSocket ACK/NACK frames**: no reply at all. A failed ACK or NACK,
  including a not-durable one, is only written to the server log.

Code: `persistence/layer.rs:858-874` (both messages),
`persistence/durable.rs:1617-1667` (`ExecError`),
`synap-core/src/core/transaction.rs:423-459` and
`synap-core/src/core/transaction.rs:509-560` (a failing command ends EXEC; the
effects so far are already applied and their records are dropped),
`synap-core/src/core/transaction.rs:424-429` (`EXECABORT`),
`server/handlers/mod.rs:387-418` and `server/handlers/mod.rs:620-636`
(envelope replies), `synap-core/src/core/error.rs:105-138` (HTTP statuses),
`server/handlers/websocket.rs:224-245` (ACK/NACK errors only logged).

---

## 3. Authorization per transport

### 3.1 Modes

| Configuration | Behavior on every transport |
|---|---|
| `auth.enabled: false` (default) | No credentials are checked and every command is allowed. |
| `auth.enabled: true`, `require_auth: false` | Requests and connections without credentials are accepted but hold **no permissions**: anything that touches data is refused. |
| `auth.enabled: true`, `require_auth: true` | HTTP requests without credentials answer `401` (except `/health` and `/metrics`); RESP3 answers `NOAUTH` until `AUTH` succeeds; SynapRPC refuses unauthenticated sessions. |

Set `auth.root.password` to at least 8 characters that are not a common
password. The built-in default (`root`) fails that check, so without it the
root user is not created at all; startup only logs a warning.

### 3.2 One policy

Every command maps to `(resource, action)` pairs, named as the REST handlers
name them: `kv:<key>`, `hash:<key>`, `list:<key>`, `set:<key>`,
`sortedset:<key>` (geospatial keys too), `hyperloglog:<key>`, `bitmap:<key>`,
`queue:<name>`, `stream:<room>`, `pubsub:<topic>`, `script:*` and `admin:*`.
The same pairs apply on every transport, so a role grants the same access
everywhere.

- Admin users pass every check. API keys are never admin.
- Keyspace-wide commands (`KEYS`, `SCAN`, `DBSIZE` and their envelope forms)
  need read on `kv:*`. Flushes, server introspection (`INFO`, the slow log, the
  client list, `admin.config`), configuration, replication and cluster commands
  are admin-only.
- A command with no policy is denied to every non-admin caller.
- Subscribing to the internal `__watch@`, `__keyspace@` and `__keyevent@`
  channels, or to a wildcard that can match them, also needs the matching `kv`
  permission.

### 3.3 Per transport

| Transport | Credentials accepted | When identity is checked | What is authorized |
|---|---|---|---|
| REST | API key (`Authorization: Bearer <key>` or `?api_key=<key>`) or HTTP Basic user and password | every request | each handler checks its pair before acting |
| `/api/v1/command` | same as REST | every request | the envelope command's pairs; an unknown command is denied |
| MCP (`/mcp`) | same as REST | every tool call uses the identity of its own HTTP request | the pairs of the matching `/api/v1/command` command; an unknown tool is denied; a call with no identity is refused when authentication is on; a session used under other credentials than the ones that opened it answers `403` |
| UMICP (`/umicp`) | same as REST | every request | same as MCP tools; a request with no identity answers `403` |
| RESP3 | `AUTH <user> <password>`, or `AUTH <password>` for the user named `default`; API keys are not accepted | once, at `AUTH` | the native command's pairs; denials are `-NOPERM` |
| SynapRPC | user and password at `AUTH`; a credential sent as a key or token is checked as the password of the user `default` | once, at `AUTH` | the native command's pairs |
| Scripts (`redis.call`) | the caller's identity | every call | the native command's pairs |
| WebSockets (KV watch, queue, stream, pub/sub) | same as REST, on the upgrade request | once, at the upgrade | KV watch: read on each key; queue: read and write on the queue; stream: read on the room; pub/sub: read on each topic |

A refused write queued into an open transaction (RESP3/SynapRPC `TXQUEUE`, or
an envelope command carrying that transaction's `client_id`) aborts the
transaction, so its EXEC applies nothing.

### 3.4 What authorization does not cover

- **Open connections keep the permissions they started with.** RESP3 and
  SynapRPC connections keep the identity captured at `AUTH`, and WebSockets the
  check made at the upgrade. Changing a user's roles, disabling or deleting the
  user does not affect a connection that is already open.
- **API keys are independent of their issuer.** A key keeps the permissions it
  was issued with, and disabling or deleting the user who created it does not
  revoke it. A key is checked for being enabled, unexpired and allowed from the
  client address only. (Issuance is bounded: a key cannot be created with a
  permission its issuer does not hold, and a key created with a temporary key
  cannot outlive it.)
- **Users and keys are not persisted** (see [1.1](#11-what-is-persisted-at-all)).
- **No transport encryption.** No listener (HTTP, RESP3, SynapRPC,
  replication) speaks TLS. Passwords and API keys travel in clear text unless
  you put a TLS-terminating proxy in front. RESP3 and SynapRPC bind to
  `127.0.0.1` by default.
- **The replication port has no authentication** (see [6.2](#62-what-replication-does-not-guarantee)).

Code: `server/router.rs:120-151` and `server/router.rs:565-661` (the three
modes), `config.rs:249-301` (defaults, root user), `main.rs:797-808`,
`auth/user.rs:48-52`, `auth/user.rs:177-183` and
`auth/password_validation.rs:23-34` (the root password must pass validation,
a failure is only a warning), `auth/access.rs:1-21`
(one policy), `auth/access.rs:155-212` (allow, deny, unknown command),
`auth/access.rs:335-346` (internal channels), `auth/access.rs:375-395`
(admin and keyspace-wide commands), `auth/middleware.rs:116-171` (API key from
header or query, never admin), `server/handlers/kv.rs:51-53` (a REST handler's
check), `server/handlers/mod.rs:351-418` (envelope), `server/mcp_server.rs:22-44`
and `server/mcp_handlers.rs:13-99` (MCP), `server/router.rs:688-849` (MCP
session binding), `server/umicp/transport.rs:15-34` (UMICP),
`protocol/resp3/server.rs:119-124` and `protocol/resp3/server.rs:186-274`
(RESP3), `protocol/synap_rpc/server.rs:100-183` and
`protocol/synap_rpc/server.rs:254-272` (SynapRPC), `scripting/mod.rs:221-233`
(scripts), `server/handlers/websocket.rs:25-56`, `:102-122`, `:285-300`,
`:476-522` (WebSockets), `server/auth_handlers.rs:144-230` (issuance),
`auth/api_key.rs:238-273` (key check), `config.rs:346-381` (loopback
defaults).

---

## 4. Hub mode

With `hub.enabled: true`:

- **Hub authentication replaces local authentication** on every HTTP route
  except `/health` and `/metrics`. The access key comes from the
  `X-Hub-Access-Key` header or `Authorization: Bearer`. A missing or rejected
  key answers `401`; a key that could not be checked answers `503`. Local users
  and API keys are not accepted.
- **Fail-closed today.** HiveHub cannot validate access keys yet
  (hivellm/synap#230). Until it can, **every request that carries a key answers
  `503`** and the server logs a warning at startup. Only `/health` and
  `/metrics` serve. Nothing is cached from a failed check.
- **Startup refuses** while the RESP3 or SynapRPC listener is enabled (both are
  on by default), naming the listener, because neither authenticates tenants.

Once keys can be validated, a request runs as its tenant:

- The identity is `hub:<tenant id>`, never an admin, with every action on
  `kv`, `hash`, `list`, `set`, `sortedset`, `hyperloglog`, `bitmap`, `queue`,
  `stream` and `pubsub` resources and nothing else.
- Names are scoped to `user_<tenant id>:` on REST, `/api/v1/command`, MCP,
  UMICP and the KV-watch, pub/sub, queue and stream WebSockets.
- Envelope, MCP and UMICP commands that cannot be confined to one tenant
  (scripts, keyspace-wide and admin commands, transactions without a string
  `client_id`) are refused.
- Routes that are not tenant-isolated answer `403`: statistics, `/info`,
  `/slowlog`, `/clients`, `/memory`, `/snapshot`, `/script/*`, REST
  `/transaction/*`, partitioned topics, consumer groups, `/cluster/*` and
  `/auth/*`. A route not on the tenant allow-list is refused by default.

Not enforced in Hub mode: plan quotas (quota fetching awaits #230), plan
payload limits and plan rate limits. HTTP rate limiting uses the `rate_limit`
settings of [5.1](#51-http-rate-limits), keyed by the tenant. Combining Hub
mode with replication only logs a warning; the replication link carries every
tenant's data.

Code: `server/router.rs:31-118` (gate, tenant routes, 401/403/503),
`hub/hub_auth.rs:88-114` (key from headers only), `hub/client.rs:23-60` and
`hub/client.rs:87-96` (validator refuses every key, startup warning),
`hub/client.rs:140-171` (only successes cached), `hub/client.rs:173-191`
(quotas unavailable), `hub/hub_auth.rs:22-36` and `hub/hub_auth.rs:69-85`
(tenant identity), `hub/multi_tenant.rs:87-89` (prefix),
`auth/access.rs:796-865` (envelope, MCP and UMICP confinement),
`config.rs:503-523` and `main.rs:247-252` (listener check),
`server/rate_limit.rs:243-262` (tenant-keyed limit), `main.rs:827-845`
(replication warning).

---

## 5. Limits

### 5.1 HTTP rate limits

Off by default (`rate_limit.enabled: false`; defaults `requests_per_second:
1000`, `burst_size: 100`). When enabled:

- Each client gets a token bucket of `burst_size` tokens refilled at
  `requests_per_second`, one token per request.
- Every request spends a token from the bucket of its **TCP peer address**,
  before authentication, so credential checks are bounded too. Forwarding
  headers such as `X-Forwarded-For` are ignored: every client behind one proxy
  shares that proxy's bucket.
- An authenticated request also spends a token from the bucket of its
  principal across all its addresses: the user (for an API key, the user who
  issued it), else the API key; in Hub mode, the tenant.
- A limited request answers `429` with `Retry-After` in whole seconds (at
  least 1). `/health` and `/metrics` are exempt.
- An enabled limiter with a zero rate or burst refuses to start.

Rate limiting covers HTTP requests only: REST, `/api/v1/command`, MCP, UMICP
and WebSocket upgrade requests. It does not cover RESP3, SynapRPC (commands or
`AUTH` attempts) or messages on an open WebSocket.

Code: `server/rate_limit.rs:1-19`, `server/rate_limit.rs:228-300`,
`server/router.rs:547-563` and `server/router.rs:663-680` (where the layers
sit), `config.rs:151-173` (validation), `config.rs:431-435` (defaults).

### 5.2 Request and connection limits

| Limit | Value | Over the limit |
|---|---|---|
| REST and `/api/v1/command` JSON body | 2 MiB (the axum default; Synap does not change it) | `413` |
| `/umicp` body | `mcp.umicp_max_body_bytes`, default 16 MiB | `413`; a declared length is refused before reading, a chunked body is cut at the limit |
| `/mcp` request body | 4 MiB (rmcp default) | refused by rmcp |
| `/mcp` `Host` header | rmcp's default allow-list: `localhost`, `127.0.0.1`, `::1` | `403`; reach `/mcp` through one of those host names |
| RESP3 bulk string / aggregate / protocol line | 512 MiB / 1,048,576 elements / 64 KiB | protocol error before allocating |
| SynapRPC frame | 512 MiB | protocol error before allocating |
| Connections per binary listener (RESP3, SynapRPC) | `network.max_connections`, default 10,000 | new connections refused |
| Idle binary connection | `network.idle_timeout_secs`, default 300 s (0 disables) | connection closed |
| Pub/Sub buffer per subscriber | 1,024 messages | the slow subscriber is disconnected |
| KV value size | `kv_store.max_value_size_bytes`, default none | `400`, **only on REST `POST /kv/set`**; other transports do not check it |

Synap sets no WebSocket message size limit of its own. See
[Network limits](network-limits.md) for the binary-listener caps in detail.

Code: `axum-core 0.5.6 src/ext_traits/request.rs:319` (2 MiB default),
`server/umicp/transport.rs:36-54` and `config.rs:226-243` (UMICP),
`rmcp 3.5.1 src/transport/streamable_http_server/tower.rs:55`, `:203` and
`:1199-1214` (MCP defaults) with `server/router.rs:781-792` (Synap passes the
default configuration), `protocol/resp3/parser.rs:150-154`,
`protocol/synap_rpc/config.rs:25`, `config.rs:81-100` (network defaults),
`synap-core/src/core/pubsub.rs:26`, `server/handlers/kv.rs:62-71` (value size
checked by this handler only).

### 5.3 Memory

Keys evicted to stay under `kv_store.max_memory_mb` (only with an eviction
policy configured; the default is `noeviction`) are removed from memory
without a WAL record, so they come back after a restart and stay on replicas.

Code: `config.rs:404` (default policy); eviction lives in
`synap-core/src/core/kv_store/store.rs`, which has no access to the
persistence layer.

---

## 6. Replication

### 6.1 What replication guarantees

- **Same records as the WAL.** Every write recorded on the shared path is
  handed to replication, also when persistence is disabled on the master.
- **Offset order.** Records get their offset and are queued for every replica
  under one lock, so replicas receive them in offset order.
- **Full sync when history is missing.** A replica that asks for offset 0, for
  an offset older than the master's in-memory backlog (1,000,000 records), or
  for an offset beyond the master's current one, gets a full sync. Otherwise it
  gets the records from its offset on.
- **A consistent full sync.** The master copies every replicated store under
  the same write barrier as snapshots, paired with its replication offset;
  writes made meanwhile are streamed after the image and applied once. The
  image covers KV (with absolute expiry), hashes, lists, sets, sorted sets,
  HyperLogLog, bitmaps, whole queues, and the stream events present at that
  moment. The replica checks the image's version and CRC32 before clearing
  anything; a load that fails part way makes the next connection ask for a full
  sync again.
- **Whole records.** A replica decodes a command only after all of its bytes
  arrived, so a transaction record is applied whole or not at all.
- **Restart means full sync.** A replica keeps its offset in memory only, so
  every replica restart starts with a full sync.
- **Reconnect.** With `auto_reconnect` (default on) a replica reconnects every
  `reconnect_delay_ms` (default 5 s), resolving `master_address` on each
  attempt.

### 6.2 What replication does not guarantee

- **Asynchronous.** The master replies without waiting for any replica, and
  replicas send no acknowledgements. A master that dies can take acknowledged
  writes with it that no replica received.
- **No master run ID.** The master's backlog and offsets restart at 0 every
  time the master starts, and the handshake carries only the replica's offset.
  A replica that reconnects after a master restart with an offset at or below
  the new master's offset gets a partial sync from the new backlog and keeps
  what it held from the previous run: it silently diverges. Restart replicas
  after restarting the master; a restarted replica asks for offset 0 and gets a
  full sync.
- **Gaps and failures are not repaired.** An offset gap is logged and the
  record applied anyway. A record that fails to apply on the replica is logged
  and skipped, and the offset still advances.
- **Replicas accept writes.** A write sent to a replica is applied there only:
  it is not forwarded to the master, and the next full sync erases it.
- **A replica's own WAL does not hold replicated records.** Only its snapshots
  (when persistence is enabled there) capture them. A replica restarted while
  the master is unreachable serves its last snapshot plus its own local writes
  until a full sync arrives.
- **Not replicated:** live stream publishes, pub/sub messages, partitioned
  topics, consumer groups, users and API keys.
- **No failover.** Nothing promotes a replica automatically.
- **No security on the link.** The replication listener
  (`replica_listen_address`) has no authentication and no encryption. Any
  client that connects and sends a handshake receives a full copy of the data;
  an unreadable handshake counts as offset 0. Bind it to a private network.
- **Lag figures are not staleness.** Heartbeats carry master offset 0, so a
  replica's lag in operations reads 0, and its lag in milliseconds is the time
  since the last heartbeat. The master's replica list shows each replica's
  offset as of its connection.
- **Expiry uses each node's clock.** Replicas keep the master's absolute
  deadlines and compare them with their own clock.

Mixed versions: a 1.3.x replica cannot decode a 1.4.0 full sync or transaction
record, so upgrade replicas before the master. A 1.4.0 replica still accepts a
1.3.x master's full sync.

Code: `persistence/layer.rs:180-209` and `main.rs:632-664` (records handed to
replication, with or without a WAL), `replication/replication_log.rs:49-80`
(offset order), `replication/master.rs:123` (backlog size),
`replication/master.rs:210-320` (handshake, full-sync rule, no
authentication), `replication/master.rs:331-376` (full sync under the
barrier), `replication/sync.rs:20-24`, `:108-148` (version, CRC32, clear then
load), `replication/replica.rs:80` (offset starts at 0),
`replication/replica.rs:100-135` (reconnect loop),
`replication/replica.rs:179-283` (sync handling, failed load resets the
offset), `replication/replica.rs:300-403` (whole commands, gaps, failed
applies), `replication/config.rs:139-153` (defaults),
`replication/master.rs:420-440` and `replication/replica.rs:405-435` (lag),
`replication/master.rs:270-290` (replica offset recorded at connection).

---

## 7. File formats and upgrades

- The WAL starts with a `SYNAPWAL` header and format version 2. A 1.3.x WAL
  (no header) replays as before and is upgraded in place on the first start
  (temporary file, fsync, rename).
- Snapshots are format v5; v4, v3 and v2 files still load.
- 1.3.x cannot read a WAL written or upgraded by 1.4.0, nor v4/v5 snapshots, so
  downgrading after 1.4.0 has written data is not supported. Back up the WAL
  and snapshot directories before upgrading.
- `synap-migrate` is the offline migration utility; this page does not cover it.

Code: `persistence/wal_frame.rs:1-50`, `persistence/snapshot.rs:19-28`,
`replication/sync.rs:20-24`.

---

## Related

- [Persistence specification](../specs/PERSISTENCE.md)
- [Snapshot format](../internals/persistence-snapshot-format.md)
- [Replication](../features/REPLICATION.md)
- [Authentication](../features/AUTHENTICATION.md)
- [Transactions](../features/transactions.md)
- [Transports](../protocol/transports.md)
- [Network limits](network-limits.md)
- [Hub configuration](../guides/HUB_CONFIGURATION.md)
