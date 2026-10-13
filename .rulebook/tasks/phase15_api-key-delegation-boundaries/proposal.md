# Proposal: phase15_api-key-delegation-boundaries

Source: docs/analysis/next-release-review-2026-10-12/ (finding S-01, priority P1)
Target release: 1.4.0

## Why

`POST /auth/keys` (`crates/synap-server/src/server/auth_handlers.rs`,
`auth_create_key`) stores whatever permissions the caller asks for. It never
checks that the issuer holds those permissions, so a read-only user can mint a
key with `{"resource":"*","action":"all"}` and use it for writes and deletes
the user itself is denied. When authentication is enabled but optional, an
anonymous caller can mint keys too. Unknown action strings silently become
`read`, and unparsable `allowed_ips` entries are silently dropped, which can
leave a key with no IP restriction at all when the caller asked for one.

## What Changes

- Require an authenticated issuer when authentication is enabled.
- Accept a requested grant only when the issuer's effective permissions cover
  it: every resource the requested pattern can match must be matched by an
  issuer grant whose action includes the requested action. Admin users may
  grant any permission.
- A key minted with an API key as issuer is bounded by that key's permissions
  and cannot outlive it.
- Reject unknown actions and unparsable `allowed_ips` entries with 400.
- A rejected request stores nothing.

## Impact

- Affected specs: auth (API key issuance)
- Affected code: `crates/synap-server/src/server/auth_handlers.rs`,
  `crates/synap-server/src/auth/permissions.rs`,
  `crates/synap-server/src/auth/api_key.rs`
- Dependencies: none
- Breaking change: YES for callers relying on the bypass. A request that asks
  for more than the issuer holds, uses an unknown action, or sends an invalid
  IP now fails with 400/403 instead of creating a key. Legitimate narrower
  grants keep working.
- User benefit: an API key can never carry more authority than the identity
  that created it.
