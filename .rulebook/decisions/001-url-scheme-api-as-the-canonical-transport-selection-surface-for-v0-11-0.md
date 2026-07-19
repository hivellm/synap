# 1. URL-scheme API as the canonical transport selection surface for v0.11.0

**Status**: proposed
**Date**: 2026-04-09
**Related Tasks**: phase3_full-rpc-resp3-parity-and-url-schemes

## Context

All five SDKs previously required separate builder fields (`transport`, `rpcHost`, `rpcPort`, `resp3Host`, `resp3Port`) to pick a transport. This led to verbose configuration, and the pattern where HTTP was "always required" even for binary transports caused confusion. Needed a cleaner, more conventional API surface.

## Decision

Encode transport choice in the URL scheme passed to the client constructor. `synap://host:port` → SynapRPC, `resp3://host:port` → RESP3, `http(s)://host:port` → HTTP. The old builder methods are kept as `#[deprecated]` shims for one release cycle and removed in v0.12.0. Native transports raise `UnsupportedCommandError`/`UnsupportedCommandException` instead of silently falling back to HTTP.

## Alternatives Considered

- Keep multi-field builder API and add URL parsing as an alternative convenience constructor
- Use enum-based transport config (SynapConfig::synap_rpc(host, port)) instead of URL strings
- Add a separate TransportConfig struct to decouple transport from the base URL

## Consequences

+ Single string encodes all connectivity info — easy to pass via env var, CLI, or config file. + Consistent across all five SDKs and the CLI. + Eliminates 'HTTP always required' confusion. - Breaking change for callers using builder methods (mitigated by one-release deprecation window). - URL scheme is non-standard for non-HTTP protocols; `synap://` and `resp3://` are Synap-specific. - `cargo audit --no-fetch` now exits 0 only after adding pre-existing advisories to ignore list.
