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

## Not yet enforced

Fine-grained per-command ACL on the binary protocols (restricting *which*
commands an authenticated non-admin may run, matching the HTTP surface) is
tracked as a follow-up (`phase6h`). Today an authenticated user may run any
command on the binary protocols; the HTTP API already enforces per-resource
permissions.
