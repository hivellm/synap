## 1. Implementation
- [x] 1.1 Install Hub authentication in the production router when `hub.enabled` is true — `router::hub_gate` replaces the local auth layer whenever `AppState.hub_client` is set; `HubClient` resolves keys through the new `AccessKeyValidator` trait (production validator fails closed, hivellm/synap#230)
- [x] 1.2 Implement the Hub mode policy: Hub credentials mandatory on data routes, explicit health and metrics exemptions — missing/rejected key 401, unverifiable key 503, local credentials rejected, `/health` and `/metrics` exempt, route allow-list `router::is_tenant_route` (403 otherwise); tenant runs as `hub:<uuid>` with data-type grants only
- [x] 1.3 Carry the tenant in `OperationContext` and make resource scoping fail closed when Hub mode has no tenant — `OperationContext::with_tenant`/`tenant_scope`, `MultiTenant::required_tenant`; envelope scoped by `auth::scope_envelope` (refuses keyspace-wide/admin/script commands); REST sorted_set, geospatial and queue stats now scoped
- [x] 1.4 Scope MCP, UMICP and WebSocket operations by tenant — MCP/UMICP tool arguments through `scope_envelope` in `handle_mcp_tool`; `/kv/ws` watch channels, `/pubsub/ws` topics, queue and stream sockets scoped
- [x] 1.5 Refuse to start in Hub mode with RESP3 or SynapRPC enabled, naming the listener in the error — `ServerConfig::validate_hub_listeners`, called from `main.rs` before any store is built
- [x] 1.6 Test the production router against a mock Hub API: missing and invalid keys, and two tenants sharing a logical key on every enabled surface — `tests/hub_tenant_isolation_tests.rs` (10 tests: 401s, REST incl. sorted set/geospatial, envelope incl. transactions, MCP Streamable HTTP, UMICP, kv-watch/pubsub/queue/stream WebSockets, 403 routes, fail-closed production validator)

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — Hub section of docs/features/AUTHENTICATION.md, docs/guides/HUB_CONFIGURATION.md
- [x] 2.2 Write tests covering the new behavior — integration test above plus unit tests in auth/access.rs, router.rs, hub/client.rs, hub/hub_auth.rs, hub/multi_tenant.rs, mcp_handlers.rs, config.rs
- [x] 2.3 Run tests and confirm they pass — fmt, clippy (all targets, full-tests, -D warnings), 474 lib tests, 18 touched integration binaries green
