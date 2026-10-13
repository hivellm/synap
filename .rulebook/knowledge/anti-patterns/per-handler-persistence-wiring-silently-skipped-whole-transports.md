# Per-handler persistence wiring silently skipped whole transports

**Category**: persistence
**Tags**: persistence, wal, replication, resp3, synaprpc, transport-parity, analysis:next-release-review-2026-10-12

## Description

Persistence (WAL + replication) is wired call-site by call-site in the REST and envelope handlers. The RESP3 and SynapRPC dispatchers were written later and call the core stores directly (`kv_store.set(...)` then reply), so every non-transactional write over the native transports was neither logged nor replicated: the only persistence call under crates/synap-server/src/protocol/ was log_transaction in EXEC. The same per-site wiring also produced D-08 (REST and envelope APPEND/SETRANGE logged `log_kv_set(key, vec![], None)`). Neither a multi-agent review nor the transport tests caught it, because the tests check replies and never restart the server or read a replica. Rules: (1) when a cross-cutting concern (persistence, authorization, notifications, tenancy) is wired per handler, list every transport's dispatcher and grep each one for the concern before trusting it; `grep -rn persistence crates/synap-server/src/protocol/` returning only EXEC was the whole proof; (2) route the concern through one shared path that every transport must call; (3) parity tests must restart from the WAL and compare a replica, not just assert the reply.

## Example

// Wrong: native dispatcher mutates and replies; WAL and replicas never see it
match state.kv_store.set(key, value, ttl).await {
    Ok(()) => Resp3Value::SimpleString("OK".into()),
    Err(e) => Resp3Value::Error(format!("ERR {e}")),
}
// Right: every transport goes through one mutation path that records the effect first
apply_and_record(&state, ctx, Mutation::KvSet { key, value, ttl }).await

## When to Use

Adding a transport, adding a cross-cutting concern, or reviewing durability/authorization claims across REST, envelope, RESP3, SynapRPC, MCP and UMICP.

## When NOT to Use

A concern implemented inside the core store itself, which every transport reaches by construction.
