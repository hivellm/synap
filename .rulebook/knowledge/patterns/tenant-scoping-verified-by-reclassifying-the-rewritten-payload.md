# Tenant scoping verified by reclassifying the rewritten payload

**Category**: security
**Tags**: hub, multi-tenant, scoping, envelope, mcp, umicp, access, phase18

## Description

Hub mode confines `/api/v1/command` payloads and MCP/UMICP tool arguments to a tenant by rewriting resource fields (`key`, `keys`, `source`, `destination`, `sources`, `source_keys`, `pairs[].key`, `queue`, `name`, `room`, `topic`, `topics`, plus `client_id`) into `user_<id>:<name>` (`auth::scope_envelope`, next to `classify_envelope`). The rewrite alone would drift from the classifier the first time a command reads a new field. So after rewriting, run `classify_envelope` on the rewritten payload and refuse the command unless every access it reports lies in the tenant's namespace. That one check fails closed for every case at once: keyspace-wide and admin accesses (`kv:*`, `admin:*`, never produced from a field), names given as numbers (not rewritten, still classified), and any field the classifier reads that the rewrite missed. Commands whose effect the classifier cannot see (script.eval/evalsha run arbitrary `redis.call`) are refused explicitly. A unit test walks every envelope command from the dispatcher table with every resource field filled in, asserting it is either refused (exactly the commands with fixed accesses or scripts) or fully confined, and pins the list of confined commands that touch no resource (transaction.*, pubsub.unsubscribe), so a new command forces a decision.

Pair it with an allow-list of HTTP routes (`router::is_tenant_route`) rather than a deny-list: a route added later answers 403 in Hub mode until someone reviews its scoping.

## Example

```rust
let prefix = MultiTenant::get_user_prefix(tenant);
rewrite_resource_fields(payload, &prefix);
let accesses = classify_envelope(command, payload).ok_or_else(|| unclassified(command))?;
match accesses.iter().find(|a| !in_namespace(&a.resource, &prefix)) {
    None => Ok(()),
    Some(a) => Err(hub_unavailable(command, format_args!("'{}' is not confined to a tenant", a.resource))),
}
```

## When to Use

Any rewrite of untrusted input that must stay in lockstep with an existing classifier or policy table: verify the rewritten input with the same classifier instead of trusting the rewrite's own field list.

## When NOT to Use

When the classifier is not authoritative about what the handler touches (the handler reads fields the classifier does not); fix the classifier first.
