# Proposal: phase16_shared-command-authorization

Source: docs/analysis/next-release-review-2026-10-12/ (findings S-02 and S-05, priority P1)
Target release: 1.4.0

## Why

Authorization depends on which transport a client picks. The dedicated REST
handlers call `require_permission` with `type:name` resources, but:

- `POST /api/v1/command` (`command_handler` in
  `crates/synap-server/src/server/handlers/mod.rs`) never extracts the
  `AuthContext`, so a read-only user can run `kv.flushall` and read or write
  any key through the envelope.
- The RESP3 and SynapRPC listeners (`protocol/resp3/server.rs`,
  `protocol/synap_rpc/server.rs`) only check `command_requires_admin`. Every
  other command, including GET, SET and DEL on any key, skips resource/action
  authorization.

Patching each entry point separately is how they drifted apart, so the fix is
one authorization contract that every transport calls.

## What Changes

- Add an `OperationContext` (principal, admin flag, effective permissions,
  client address, and a tenant slot that phase18 fills) built once per request
  or connection from the REST `AuthContext`, the RESP3 `AUTH` user, or the
  Thunder session principal.
- Add one command access classifier that maps a command and its arguments to
  the `(resource, action)` pairs it touches: every source and destination key,
  every key of multi-key commands, declared script keys, each command queued in
  a transaction, and subscribe/watch channels and keys. Resource names reuse
  the REST naming (`kv:<key>`, `hash:<key>`, `queue:<name>`, ...) so a role
  grants the same access on every transport. Keyspace-wide commands (FLUSHALL,
  FLUSHDB, KEYS, SCAN, DBSIZE, stats) carry an explicit policy.
- Authenticated mode fails closed: a command without a classification is
  denied, and a test fails when any dispatcher command lacks one, so a new
  command cannot skip authorization by omission.
- The envelope endpoint, RESP3 and SynapRPC authorize before dispatch. The
  admin-command gate stays.
- Commands issued from inside a script are authorized against the caller's
  context. A transaction with a denied queued command aborts at EXEC without
  applying anything.
- With authentication disabled the server stays unrestricted, as an explicit
  mode rather than a side effect of a missing context.

## Impact

- Affected specs: auth (command authorization)
- Affected code: `crates/synap-server/src/auth/`,
  `crates/synap-server/src/server/handlers/mod.rs`,
  `crates/synap-server/src/protocol/resp3/`,
  `crates/synap-server/src/protocol/synap_rpc/`, the scripting and
  transaction modules
- Dependencies: phase15 (shares the permission coverage rules)
- Breaking change: YES for deployments with authentication enabled whose
  clients relied on the envelope or native listeners ignoring ACLs. Those
  calls now fail with the same denial REST returns.
- User benefit: one role means the same thing on REST, the command envelope,
  RESP3 and SynapRPC.
