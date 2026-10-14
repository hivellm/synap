---
title: Upgrading to 1.4.0
module: guides
id: upgrading-to-1-4
order: 8
description: What changes when a 1.3.x deployment moves to Synap 1.4.0, who is affected, and what to do
tags: [guides, upgrade, migration, release, 1.4.0]
---

# Upgrading to 1.4.0

1.4.0 is a hardening release. It reads every file a 1.3.x server wrote, but the
first time it starts it converts the WAL to a new format, and its snapshots use
newer formats too. **1.3.x cannot read data that 1.4.0 has written, so going
back to 1.3.x means restoring a backup taken before the upgrade.** Several
checks that 1.3.x skipped are now enforced: authorization on every transport,
configured rate limits, Hub tenant isolation and the UMICP body size.

Read [Before you upgrade](#before-you-upgrade) first, then the sections that
match your deployment. Each breaking change lists what changed, who is
affected, and what to do. [`CHANGELOG.md`](../../../CHANGELOG.md) has the full
list of changes.

## Checklist

| If you run... | Do this before upgrading |
|---|---|
| Any server with persistence | Stop it and back up the WAL **and** the snapshot directory together ([details](#before-you-upgrade)) |
| Replicas | Upgrade every replica first, the master last ([details](#replication-upgrade-replicas-before-the-master)) |
| `auth.enabled: true` | Check that each client holds the permissions it uses, on the transport it uses ([details](#authorization-on-every-transport)) |
| API keys with `allowed_ips` | List the addresses clients really connect from ([details](#api-key-allowed_ips-now-sees-the-real-client-address)) |
| `hub.enabled: true` | Disable RESP3 and SynapRPC; tenant requests answer 503 until hivellm/synap#230 ships ([details](#hub-mode)) |
| `rate_limit.enabled: true` | Check the limits: they are now enforced ([details](#rate-limiting-is-enforced)) |
| UMICP clients | Bodies above 16 MiB answer 413 ([details](#umicp-request-body-limit)) |
| Scripts that call `synap-cli PING` | A failed ping now exits 1 ([details](#synap-cli-ping-exit-status)) |
| The desktop GUI | The Configuration view is read-only and needs an admin ([details](#gui-configuration-view-is-read-only)) |
| Queue consumers | Make sure redelivery after a restart is handled ([details](#queue-redelivery-after-a-restart)) |
| Any client SDK | Upgrade it for correct `INCR`/`DECR` amounts ([details](#sdks)) |

## Before you upgrade

### Back up the WAL and snapshots together

**What changed.** On its first start, 1.4.0 rewrites the WAL with a format
header (see [WAL and snapshot formats](#wal-and-snapshot-formats)), and the
first snapshot it takes uses format v5. A 1.3.x server cannot read either file.

**Who is affected.** Every deployment with `persistence.enabled: true`.

**What to do.** Stop the server, then copy the WAL file and the snapshot
directory as one backup, taken at the same moment. A snapshot restored without
the WAL written after it, or the other way round, does not give you the state
the server had.

- WAL file: `persistence.wal.path` (the shipped configurations use
  `/data/wal/synap.wal`).
- Snapshot directory: `persistence.snapshot.directory` (the shipped
  configurations use `/data/snapshots`).

On a host install, with the shipped paths:

```bash
systemctl stop synap
tar czf synap-1.3-backup.tar.gz -C / data/wal data/snapshots
```

With the Docker image, whose data lives in the `/data` volume:

```bash
docker stop synap
docker run --rm -v synap-data:/data -v "$(pwd)":/backup \
  alpine tar czf /backup/synap-1.3-backup.tar.gz -C /data .
```

Keep the backup until you are sure you will stay on 1.4.0.

### There is no downgrade

Downgrading after 1.4.0 has written data is not supported. To go back to 1.3.x:
stop 1.4.0, restore the WAL and snapshot directory from the backup above (both,
from the same backup), and start 1.3.x. Every write made after the upgrade is
lost. Do not start 1.3.x on a data directory 1.4.0 has used.
[Backup and Restore](../operations/BACKUP.md) describes the restore steps.

## WAL and snapshot formats

| File | Written by 1.3.x | Written by 1.4.0 | 1.4.0 reads |
|---|---|---|---|
| WAL | No header: frames start at byte 0 (format 1) | 12-byte header: `SYNAPWAL` plus format version 2, then frames | Formats 1 and 2 |
| Snapshot | v3, named `snapshot-v3-<unix seconds>.bin` | v5, named `snapshot-v5-<20-digit sequence>.bin` | v2, v3, v4 and v5 |

What happens on the first 1.4.0 start with 1.3.x data:

1. Recovery loads the newest snapshot and replays the WAL after it. Files with
   the old timestamp names load, ordered before the new sequence-numbered ones.
2. Before the first new WAL record is written, the 1.3.x WAL is upgraded in
   place: the header and the unchanged frames go to a temporary file
   (`<wal path>.upgrade`), which is fsynced and renamed over the original. A
   crash during the upgrade leaves either the untouched 1.3.x file or the
   complete new one. The log says
   `upgraded from format version 1 (Synap 1.3.x, no header) to version 2`.
3. The first snapshot 1.4.0 takes is v5. Older snapshot files stay until
   retention (`max_snapshots`) removes them.

Expiry on that first start: keys restored from a 1.3.x (v3) snapshot come back
without expiry, as they did when 1.3.x restarted. A TTL recorded in the 1.3.x
WAL now counts from the time the record was written, not from the restart, so a
key that expired while the server was down is not restored. From then on,
snapshots (v4 and later) and new WAL records store each key's absolute
deadline.

### Startup now refuses damaged or unknown data

**What changed.** Where 1.3.x could start with empty or partial data, 1.4.0
refuses to start and names the file:

- a WAL whose header declares a newer format than 2 (the error names both
  versions), or whose header is damaged;
- a WAL with interior corruption: a bad frame followed by valid ones (the error
  gives the byte position). A torn tail with nothing valid after it, which a
  crash in the middle of a write leaves, is still truncated automatically;
- a damaged newest snapshot when the WAL does not cover an older one. When the
  WAL does cover it, recovery falls back to the older snapshot.

**Who is affected.** Deployments whose data files are damaged, which 1.3.x
could hide by losing data silently.

**What to do.** Do not delete files to get the server up. Restore the WAL and
snapshots from a backup, or keep the files for investigation.

### Writes fail when they cannot be made durable

**What changed.** With persistence on, every transport now records each write
in the WAL and sends it to replicas. 1.3.x skipped that for RESP3, SynapRPC,
scripts, MCP and most data-type endpoints, so those writes were lost on
restart. If the WAL cannot record a write, the client gets an error saying the
write is applied in memory but not durable. A failed WAL write, flush or fsync fails every acknowledgement waiting
on it and stops the WAL from accepting writes until the server restarts; while
that lasts, `GET /health` answers 503 with `"wal": "poisoned"`.

**Who is affected.** Deployments whose WAL device fails or fills up, and
health checks that probe `/health`.

**What to do.** Treat a 503 from `/health` as a failed node. Fix the disk and
restart the server.

### Backup tooling

Snapshot file names changed to `snapshot-v<format>-<sequence>.bin`. A script
that selects `snapshot-v3-*` files must match `snapshot-v*-*.bin` instead.
Recovery picks the highest sequence number, not the newest file date.

## Replication: upgrade replicas before the master

**What changed.** A 1.4.0 master sends records a 1.3.x replica cannot decode: a
committed transaction is now one record, and full sync uses a new, versioned
command that carries every datatype (hashes, lists, sets, sorted sets,
HyperLogLogs, bitmaps and queues, not only KV keys and streams). A 1.4.0
replica still accepts a full sync from a 1.3.x master, which carries KV keys
and streams only, as before.

**Who is affected.** Every master-replica deployment.

**What to do.**

1. Back up the master's WAL and snapshots ([see above](#before-you-upgrade)).
2. Upgrade each replica: stop it, install 1.4.0, start it. It reconnects to the
   1.3.x master.
3. Upgrade the master last.
4. Watch the master's log as the replicas reconnect: it logs
   `Performing full sync for replica` or `Performing partial sync for replica`
   for each one.

Do not run a 1.4.0 master with 1.3.x replicas. A 1.4.0 replica that receives a
full-sync format it does not know refuses it with an error that says to upgrade
replicas before the master.

`replication.master_address` now also accepts a DNS name (`master:15600`), and
`SYNAP_MASTER_ADDRESS` overrides it. Existing `IP:port` values keep working.

## Authorization on every transport

**What changed.** 1.3.x checked permissions on some REST routes only. In 1.4.0,
every command on every transport is checked against one policy: each command
names the resources it touches and what it does to them, and the caller must
hold a permission for each pair. A command with no policy is denied.

Resources are named the same way on every transport:

| Resource | Covers |
|---|---|
| `kv:<key>` | Strings and keys |
| `hash:<key>`, `list:<key>`, `set:<key>` | Hashes, lists, sets |
| `sortedset:<key>` | Sorted sets and geospatial keys |
| `hyperloglog:<key>`, `bitmap:<key>` | HyperLogLogs, bitmaps |
| `queue:<name>`, `stream:<room>`, `pubsub:<topic>` | Queues, streams, pub/sub topics |
| `script:*` | Script management |
| `admin:*` | Administration (needs the `admin` action) |

Actions are `read`, `write`, `delete`, `configure`, `admin` and `all`.
Keyspace-wide commands (`KEYS`, `SCAN`, `DBSIZE`, `kv.stats`) need `read` on
`kv:*`.

What each transport checks:

| Transport | Identity | What is checked | A refusal looks like |
|---|---|---|---|
| REST routes | Each request's credentials | Every handler checks the resources it touches. 49 handlers that checked nothing in 1.3.x now do: sorted sets, geospatial, partitions and consumer groups, scripts, queue ACK/NACK, the slow log, the client list and snapshots. `/info`, `/slowlog` and `/clients` need admin. | HTTP 403 |
| `POST /api/v1/command` (envelope) | Each request's credentials | Every command. `info`, `slowlog.get`, `slowlog.reset`, `client.list`, `admin.config`, `kv.flushdb` and `kv.flushall` need admin; `ping` needs nothing. A refused write to an open transaction aborts it, so its `EXEC` applies nothing. | HTTP 200 with `"success": false` and an error starting `Forbidden: NOPERM` |
| MCP (`/mcp`) | The credentials of the HTTP request carrying the tool call | Every tool (24), with the permissions of the envelope command it runs: `synap_kv_set` needs what `kv.set` needs. With authentication enabled, a call without an identity is refused. A legacy MCP session works only under the credentials that opened it. | Tool error with `NOPERM`; HTTP 403 for a session used under other credentials |
| UMICP (`/umicp`) | The credentials of the HTTP request | Same as MCP: each operation is authorized as its tool | Error envelope with `NOPERM` |
| RESP3 | The user given to `AUTH`, captured at `AUTH` | Every command. `FLUSHALL`, `FLUSHDB`, `CONFIG`, `SHUTDOWN`, `CLUSTER`, `SCRIPT FLUSH`/`KILL` and the other server commands need admin. | `-NOPERM` error |
| SynapRPC | The session's `AUTH` identity, captured at `AUTH` | Every command, with the same policy as RESP3 | Error reply starting `NOPERM` |
| Scripts (`EVAL`, `redis.call`) | The caller of the script | Every `redis.call` inside the script, with the caller's permissions | The script call fails with `NOPERM` |
| WebSockets | The upgrade request's credentials | `/kv/ws`: `read` on `kv:<key>` for every watched key. `/queue/{name}/ws/{consumer}`: `read` and `write` on `queue:<name>`. `/stream/{room}/ws/{subscriber}`: `read` on `stream:<room>`. `/pubsub/ws`: `read` on `pubsub:<topic>` for every topic. | HTTP 403 on the upgrade |

Pub/sub: subscribing to an internal channel (`__watch@...`, `__keyspace@...`,
`__keyevent@...`), or to a wildcard that can match one, also needs `read` on
the matching `kv` resource (`kv:*` for `__keyevent@` and for wildcards).

**Who is affected.** Deployments with `auth.enabled: true`. In particular:

- clients whose user or API key holds fewer permissions than the commands they
  send over the envelope, RESP3, SynapRPC, MCP, UMICP or WebSockets: 1.3.x let
  most of those commands through, 1.4.0 refuses them;
- with `auth.require_auth: false`: a RESP3 or SynapRPC connection that never
  sends `AUTH` now holds no permissions, the same as an anonymous REST request,
  so it can run only commands that touch no data, such as `PING` and `AUTH`;
- monitoring that reads `/info`, `/slowlog` or `/clients` with a non-admin
  credential.

With `auth.enabled: false` nothing changes: every transport stays
unrestricted. Hub mode replaces local authentication; see [Hub mode](#hub-mode).

**What to do.**

1. List every client, the transport it uses and the commands it sends.
2. Give each one the permissions it needs: an API key with explicit grants
   (`POST /auth/keys` with `permissions: [{"resource": "queue:jobs", "action":
   "write"}]`), or a role on its user (`admin` holds everything, `readonly`
   holds `read` on `*`).
3. Clients of RESP3 or SynapRPC that relied on anonymous access must `AUTH`.
4. Test each client with its real credentials against a 1.4.0 staging server
   before upgrading production.

RESP3 and SynapRPC resolve a connection's permissions once, at `AUTH`. After
you change a user's roles, those clients get the new permissions when they
reconnect or authenticate again. REST, envelope, MCP and UMICP requests are
checked with the permissions current at the time of each request.

See [Authentication](../../features/AUTHENTICATION.md) for users, roles and API
keys.

### API key issuance is bounded by the issuer

**What changed.** `POST /auth/keys` stored whatever permissions it was asked
for. Now:

- every requested grant must be covered by one permission the issuer holds
  (action included, resource pattern contained); otherwise 403. Admins may
  grant anything;
- with authentication enabled, an unauthenticated request answers 401, even
  when `require_auth` is off;
- a key created with a temporary API key may not hold more than that key and
  must expire no later than it;
- an unknown action, an empty resource or an unparsable `allowed_ips` entry
  answers 400. 1.3.x turned an unknown action into `read` and dropped a bad
  address (and an empty `allowed_ips` list allows every address).

**Who is affected.** Non-admin users and automation that create API keys.

**What to do.** Create keys as an admin, or as a user who holds every
permission the key needs. Fix requests that send action names other than
`read`, `write`, `delete`, `configure`, `admin` or `all`.

### API key `allowed_ips` now sees the real client address

**What changed.** The 1.3.x HTTP server did not pass the connection's address
to authentication, so every HTTP client looked like `127.0.0.1`. 1.4.0 checks
`allowed_ips` against the TCP peer address. `X-Forwarded-For` and similar
headers are ignored, so behind a reverse proxy the address is the proxy's.

**Who is affected.** API keys with a non-empty `allowed_ips` list. A key
limited to `127.0.0.1` used from another host worked in 1.3.x and is refused
now.

**What to do.** Put the addresses clients (or your proxy) really connect from
in `allowed_ips`.

## Hub mode

**What changed.** With `hub.enabled: true`:

- **The server refuses to start while RESP3 or SynapRPC is enabled.** Both are
  enabled by default, and neither can authenticate a Hub tenant. The error
  names the listener.
- Hub authentication replaces local authentication on every route except
  `/health` and `/metrics`. A request needs a Hub access key, in
  `X-Hub-Access-Key` or `Authorization: Bearer`. Without one, or with one the
  Hub rejects, the answer is 401; local users and API keys are not accepted.
- **Every request with an access key answers 503 for now.** HiveHub cannot
  validate access keys yet (hivellm/synap#230), so the server fails closed
  instead of letting any key through, and logs a warning saying so at startup.
- Once keys can be validated, each tenant runs with data-type permissions only,
  and its names are scoped to `user_<id>:` on REST, `/api/v1/command`, MCP,
  UMICP and the WebSockets. Routes that cannot be confined to one tenant
  (statistics, `/info`, `/slowlog`, `/clients`, `/memory`, `/snapshot`,
  `/script/*`, REST `/transaction/*`, partitioned topics, consumer groups,
  `/cluster/*`, `/auth/*`) answer 403.

**Who is affected.** Deployments with `hub.enabled: true` or
`SYNAP_HUB_ENABLED=true`.

**What to do.** Set `resp3.enabled: false` and `synap_rpc.enabled: false`.
Plan for tenant traffic to be refused with 503 until hivellm/synap#230 is
resolved: upgrading a Hub deployment stops its tenant traffic until then.

1.3.x never isolated tenants: data written in Hub mode by 1.3.x sits under
unscoped names that no 1.4.0 tenant can reach. Moving it under a tenant's
`user_<id>:` prefix is a job for synap-migrate.

## Rate limiting is enforced

**What changed.** In 1.3.x, `rate_limit.enabled: true` was logged as enabled
but no limit was applied. 1.4.0 applies it to the HTTP server: REST,
`/api/v1/command`, MCP, UMICP and the WebSocket upgrades. RESP3 and SynapRPC are
not rate limited.

- Each client gets a token bucket that holds `burst_size` requests and refills
  at `requests_per_second`.
- Every request spends a token from the bucket of its TCP peer address, before
  authentication, so failed password and API key checks are limited too. A
  request that authenticates also spends one from the bucket of its user, API
  key or Hub tenant, so one principal is limited across all its addresses.
- A limited request answers 429 with a `Retry-After` header (whole seconds).
- `/health` and `/metrics` are never limited.
- With `enabled: true`, a zero `requests_per_second` or `burst_size` stops the
  server at startup.
- The startup log says whether rate limiting is enforced.

**Who is affected.** Deployments with `rate_limit.enabled: true`. Of the
shipped files, `config/config-cluster.yml` enables it (1000 requests per
second, burst 100). Behind a reverse proxy, every client shares the proxy's
address and therefore one bucket.

**What to do.** Size `requests_per_second` and `burst_size` for your real
traffic, or set `enabled: false`. Make clients retry after `Retry-After` on
429. See [Rate Limiting](../configuration/RATE_LIMITING.md).

## UMICP request body limit

**What changed.** `POST /umicp` reads at most `mcp.umicp_max_body_bytes`
(default 16 MiB, `16777216`) and answers 413 above it. A declared
`Content-Length` over the limit is refused before the body is read; a chunked
body stops being read at the limit.

**Who is affected.** UMICP clients that send larger envelopes.

**What to do.** Raise `mcp.umicp_max_body_bytes`, or split the payload. The
other connection and request limits are unchanged; see
[Network Limits](../../operations/network-limits.md).

## `synap-cli PING` exit status

**What changed.** `synap-cli PING` used to print "Server unreachable" and exit
0. It now sends a real protocol ping (`PING` over SynapRPC and RESP3, the
`ping` command of `/api/v1/command` over HTTP) and exits 1, with the reason on
stderr, when the connection is refused, the request times out or the
credentials are rejected. Every command run non-interactively exits 1 on
failure, and errors go to stderr.

New flags: `--user` and `--password` (HTTP Basic auth, or the SynapRPC
handshake; not supported on `resp3://` URLs) and `--timeout` in seconds
(default 30). Over HTTP the ping goes through authentication: wrong
credentials always fail it, and with `auth.require_auth: true` it needs
credentials.

**Who is affected.** Scripts and health checks that run `synap-cli PING` and
read its exit status or output.

**What to do.** Treat exit status 1 as down. Pass `--user`/`--password` where
authentication is required. For a liveness probe that needs no credentials, use
`GET /health` or `synap-server --health-check`.

## GUI configuration view is read-only

**What changed.** The desktop GUI's Configuration view showed a built-in sample
instead of the server's configuration, and its Save never reached the server.
It now shows the server's effective configuration (file, environment and
command-line overrides applied), with passwords, API keys, the Hub service key
and other secret fields replaced by `[redacted]`. You can copy or download it
as YAML, but not edit it. The data comes from the new admin-only
`admin.config` command of `/api/v1/command`, which is refused in Hub mode.

**Who is affected.** GUI users who changed the configuration from the GUI, and
GUI users without admin permission, who now see a permission error in that
view.

**What to do.** Change the configuration in the file the server starts from (or
its environment variables) and restart the server. Connect the GUI with an
admin credential to see the configuration.

## Queue redelivery after a restart

**What changed.** In 1.3.x, a restart (WAL replay or snapshot load) and a
replica gave every queue message a new ID, so an ACK recorded before the
restart no longer matched and acknowledged messages came back deliverable. In
1.4.0:

- messages keep their ID, priority, retry counters, creation time and headers
  across restarts and on replicas, and recorded ACKs and NACKs settle by ID, so
  acknowledged messages stay acknowledged;
- messages that were delivered but not acknowledged when the server stopped
  come back ready, ahead of waiting messages of the same or lower priority, in
  the order they were delivered;
- queue creation (with its configuration), deletion and purge survive restarts
  and reach replicas; dead-lettered messages and queue configuration are kept in
  snapshots (v5);
- a message requeued because its ACK deadline passed is recorded as a NACK, so
  retry counts and dead-lettering match after a restart.

Delivery stays at-least-once. An ACK sent after a restart for a message
delivered before it answers not found (404 over REST), and the message is
delivered again.

**Who is affected.** Queue consumers that are connected across a restart, or
whose processing spans one.

**What to do.** Make consumers idempotent, keyed by message ID, which now
stays the same across restarts. Treat a not-found ACK after a restart as
expected: the message has been or will be redelivered.

## SDKs

**What changed.** Older SDKs sent a one-argument `INCR`/`DECR` over SynapRPC
and RESP3, so an amount was ignored and the counter moved by 1. Over HTTP, the
C# and PHP SDKs sent the amount as `delta` and the Python SDK as `kv.incrby`,
which the HTTP API never read, with the same result. The 1.4.0 SDKs send
`amount` on HTTP and `INCRBY`/`DECRBY` on SynapRPC and RESP3, so `INCR count
10` adds 10 on every transport. `kv.incr` and `kv.decr` on `/api/v1/command`
now answer an error for an `amount` that is not a 64-bit integer, instead of
treating it as 1.

**Who is affected.** Applications that increment or decrement by more than 1
through an older SDK.

**What to do.** Upgrade the SDKs together with the server.

## Related topics

- [Backup and Restore](../operations/BACKUP.md)
- [Persistence](./PERSISTENCE.md)
- [Replication](./REPLICATION.md)
- [Authentication](../../features/AUTHENTICATION.md)
- [Rate Limiting](../configuration/RATE_LIMITING.md)
- [Network Limits](../../operations/network-limits.md)
