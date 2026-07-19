# Proposal: phase6i_v1-connection-idle-timeout-and-configurable-limits

Source: docs/analysis/synap-audit/ (hardening beyond M-015); follow-up of phase6c

## Why
phase6c closed the critical DoS vectors from the audit (unbounded allocation on
RESP3/SynapRPC, unbounded pub/sub buffers, unbounded concurrent connections) with
sane hard-coded caps. Two hardening items were deferred: (1) an idle-connection
timeout — with only a max-connections cap, a slow-loris client can still hold all
10,000 slots by connecting and never sending; (2) making every limit configurable
so operators can tune them per deployment instead of recompiling. Both improve
robustness but were not among the explicit audit findings.

## What Changes
1. Wrap the RESP3 and SynapRPC per-connection read with a configurable idle
   timeout; close a connection that sends nothing within the window.
2. Move the network-limit constants (MAX_BULK_LEN, MAX_AGGREGATE_LEN,
   MAX_LINE_LEN, MAX_FRAME_SIZE, SUBSCRIBER_CHANNEL_CAPACITY, MAX_CONNECTIONS,
   idle timeout) into config with the current values as defaults; thread them
   from `config.rs` into the parser/codec/pubsub/listener paths.
3. Document the knobs in config.example.yml (docs/network-limits.md already
   lists the defaults).

## Impact
- Affected specs: configurable network limits + idle timeout (ADDED)
- Affected code: `crates/synap-server/src/protocol/{resp3,synap_rpc}/server.rs`,
  `synap-protocol` parser/codec (accept a limit parameter), `synap-core` pubsub,
  `config.rs`, `config.example.yml`
- Breaking change: NO (defaults preserve current behaviour)
- User benefit: slow-loris resistance and per-deployment tuning of all resource
  limits without recompiling
