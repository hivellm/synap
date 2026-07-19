# Proposal: phase6h_v1-binary-protocol-per-command-acl

Source: docs/analysis/synap-audit/ (M-003, M-004 — ACL refinement); follow-up of phase6b

## Why
phase6b closed the critical hole (RESP3 and SynapRPC accepted commands with no
authentication) by requiring a successful AUTH before any command. It did NOT yet
enforce per-command ACL: an authenticated non-admin user can currently run any
command on the binary protocols, whereas the HTTP path already checks
per-resource/per-action permissions (`require_permission`,
`require_resource_permission`). This follow-up brings the binary protocols to
parity so a user's role/ACL actually restricts which commands they may run
(e.g. only admins run FLUSHALL/CONFIG).

## What Changes
1. Define a command → (resource, action) permission map covering the RESP3
   94-command dispatch and the SynapRPC command set (or a shared table both use).
2. After authentication, resolve the connection's `User`/roles and check the
   required permission before executing each command on both protocols; deny with
   an ACL error otherwise.
3. Gate destructive/admin commands (FLUSHALL, FLUSHDB, CONFIG, cluster ops)
   behind admin at minimum.
4. Tests: authenticated non-admin denied an unpermitted command on each protocol;
   admin allowed.

## Impact
- Affected specs: ACL enforcement on binary protocols (ADDED)
- Affected code: `crates/synap-server/src/protocol/resp3/{server,command}`,
  `protocol/synap_rpc/{server,dispatch}`, `auth/` permission mapping
- Breaking change: NO for admins; authenticated non-admins gain correct restrictions
- User benefit: least-privilege actually enforced on the Redis-compatible and
  native binary ports, matching the HTTP surface
