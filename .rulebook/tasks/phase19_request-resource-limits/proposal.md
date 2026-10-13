# Proposal: phase19_request-resource-limits

Source: docs/analysis/next-release-review-2026-10-12/ (findings S-06 and S-07, priority P2)
Target release: 1.4.0

## Why

- Rate limiting is configurable but not enforced. `server/router.rs` logs that
  the limiter "requires middleware integration", while `main.rs` logs that rate
  limiting is enabled. Operators believe traffic, including bcrypt-heavy
  authentication attempts, is bounded when it is not.
- UMICP reads the whole request body with
  `axum::body::to_bytes(request.into_body(), usize::MAX)`
  (`server/umicp/transport.rs`). There is no application limit, so one client
  can make the server buffer arbitrarily large bodies before parsing.

## What Changes

- Install the configured limiter as router middleware, keyed by the
  authenticated principal when present and the peer address otherwise, and
  placed before authentication so password checks are limited too.
- Answer 429 with `Retry-After` when limited. Health probes are exempt and the
  exemption is documented.
- Startup logs state whether rate limiting is actually enforced.
- Bound UMICP request bodies with a configurable limit and a documented
  default, answering 413 without reading the rest of the body.

## Impact

- Affected specs: server (request limits)
- Affected code: `crates/synap-server/src/server/router.rs`,
  `crates/synap-server/src/main.rs`, `crates/synap-server/src/server/umicp/`,
  config
- Dependencies: phase18 (identity policy decides what a principal is in Hub mode)
- Breaking change: YES for deployments that set `rate_limit.enabled: true`,
  since the limit now applies. Oversized UMICP bodies are rejected.
- User benefit: configured limits mean what they say.
