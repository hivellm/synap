# Proposal: phase6b_v1-protocol-auth-hardening

Source: docs/analysis/synap-audit/ (M-003, M-004, M-009)

## Why
Both binary protocols accept commands with no authentication, and password storage is weak.
(M-003) The SynapRPC dispatcher maps commands straight to store operations with no auth/ACL
(`protocol/synap_rpc/dispatch/mod.rs:19-47`); the listener (default port 15501) accepts
`SET/GET/DEL/FLUSHALL/KEYS` from any client. (M-004) RESP3 auth is a stub — `check_auth`
always returns `true` and the connection starts `authenticated = true`
(`protocol/resp3/server.rs:75,212-216`) — so port 6379 executes every command unauthenticated
regardless of configured users. (M-009) Passwords are hashed with unsalted single-round SHA-512
and compared with a short-circuiting `==` (`auth/user.rs:54-64`), vulnerable to rainbow tables
and timing attacks — while `bcrypt` is already a workspace dependency and unused here. For a
1.0 datastore these are critical security holes.

## What Changes
1. Thread the existing `AppState` auth (user manager + ACL + api keys) into SynapRPC dispatch:
   a HELLO/AUTH handshake per connection, an authenticated flag, and a per-command permission
   check before executing store operations. Unauthenticated commands are rejected.
2. Wire `AppState` auth into RESP3 `check_auth` and default the connection's `authenticated`
   to `!auth.required`; make AUTH validate a real user/password and enforce ACL on dispatch.
3. Replace SHA-512 password hashing with bcrypt (already a dependency) using a per-user salt;
   compare with a constant-time equality. Provide a migration path for existing SHA-512 hashes
   (verify-and-rehash on next successful login, or a one-shot migration in synap-migrate).

## Impact
- Affected specs: auth enforcement on RESP3 and SynapRPC (ADDED); password hashing (MODIFIED)
- Affected code: `crates/synap-server/src/protocol/synap_rpc/dispatch/mod.rs`,
  `protocol/resp3/server.rs`, `auth/user.rs`, possibly `synap-migrate` for hash migration
- Breaking change: YES — clients on the binary protocols now must authenticate when auth is
  enabled; existing password hashes migrate on next login
- User benefit: the Redis-compatible and native binary listeners are no longer open doors;
  credentials are stored with a real password-hashing function
