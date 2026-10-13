## 1. Implementation
- [x] 1.1 Enforce configured rate limits as router middleware keyed by principal or peer address, placed before authentication — `rate_limit::limit_by_peer` (TCP peer IP, outside auth) + `limit_by_principal` (user / API key / `hub:<uuid>`, inside auth); `main` now serves with `into_make_service_with_connect_info`
- [x] 1.2 Answer 429 with `Retry-After` and exempt health probes explicitly — whole seconds, rounded up, >= 1; `/health` and `/metrics` exempt via `is_public_path`
- [x] 1.3 Make startup logs report whether rate limiting is actually enforced — router logs `Rate limiting enforced: ...` only when it installs the layers, else `Rate limiting disabled: ...`; duplicate claim in `main.rs` removed; invalid enabled config (0 rate or burst) refuses to start
- [x] 1.4 Bound UMICP request bodies with a configurable limit and answer 413 above it — `mcp.umicp_max_body_bytes` (default 16 MiB); Content-Length over the limit refused before reading, chunked bodies stop at the limit
- [x] 1.5 Test burst, refill, per-client independence, limited authentication attempts, and oversized UMICP bodies with Content-Length and chunked encoding — `tests/request_limits_tests.rs` (11 tests) + unit tests in `rate_limit.rs` and `config.rs`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — `docs/users/configuration/RATE_LIMITING.md`, `CONFIGURATION.md`, `docs/protocol/UMICP_INTEGRATION.md`, `docs/users/api/UMICP.md`
- [x] 2.2 Write tests covering the new behavior — see 1.5
- [x] 2.3 Run tests and confirm they pass — fmt, clippy `--all-targets --features full-tests -D warnings`, lib (504), request_limits (11), auth_permission_integration (20), authorization_matrix (6), hub_tenant_isolation (10), mcp_identity (3), config (9): all green
