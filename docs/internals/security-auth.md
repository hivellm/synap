# Authentication & password security

## Password hashing

User passwords are hashed with **bcrypt** (`bcrypt::DEFAULT_COST`), which embeds a
random per-user salt. `User::verify_password`:

- verifies bcrypt hashes (prefix `$2`) via bcrypt's constant-time check;
- verifies any remaining legacy unsalted SHA-512 hash with a constant-time
  comparison, and `UserManager::authenticate` transparently re-hashes it to
  bcrypt on the next successful login (`User::needs_rehash`).

Plain unsalted SHA-512 and short-circuiting `==` comparison are no longer used
(audit M-009).

## Authentication on the binary protocols

Both binary listeners enforce authentication when `config.auth` requires it
(audit M-003, M-004). `AppState` carries the optional `user_manager` and a
`require_auth` flag:

- **RESP3** (port 6379): a connection starts unauthenticated when auth is
  required. `AUTH <password>` (default user) or `AUTH <user> <password>` is
  validated against the user manager; other commands are rejected with `NOAUTH`
  until authentication succeeds.
- **SynapRPC** (port 15501): the connection loop handles `AUTH` inline (same
  credential forms) and rejects every other command with `NOAUTH` until the
  connection authenticates.

When auth is disabled, `user_manager` is `None` and connections start
authenticated (no behaviour change for local/dev use).

## Per-command ACL on the binary protocols (phase6h)

After authentication, both the RESP3 and SynapRPC dispatchers gate
**destructive and administrative commands behind admin**: an authenticated
non-admin user issuing one of them is denied with `NOPERM`. The gated set
(`auth::command_requires_admin`) is:

- keyspace/database wipes — `FLUSHALL`, `FLUSHDB`, `SWAPDB`
- server admin/config — `CONFIG`, `SHUTDOWN`, `DEBUG`, `RESET`, `SAVE`,
  `BGSAVE`, `BGREWRITEAOF`, `LASTSAVE`, `FAILOVER`, `SLAVEOF`, `REPLICAOF`,
  `ACL`, `MODULE`
- script-cache management — `SCRIPT.FLUSH`, `SCRIPT.KILL` (running a script is
  not admin)
- cluster administration — `CLUSTER`

The connection tracks the resolved `User` from AUTH and checks `is_admin` before
executing these commands. **When authentication is disabled** the binary ports
are trusted (loopback by default, per phase6 bind hardening) and no restriction
is applied.

Finer-grained per-resource ACL (e.g. read/write on a specific key pattern,
matching the HTTP surface's `require_permission`) beyond the admin gate remains
a potential future refinement; the admin gate covers the dangerous commands.
