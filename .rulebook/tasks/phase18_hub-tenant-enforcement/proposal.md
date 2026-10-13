# Proposal: phase18_hub-tenant-enforcement

Source: docs/analysis/next-release-review-2026-10-12/ (finding S-03, priority P1)
Target release: 1.4.0

## Why

Hub mode does not isolate tenants. `main.rs` creates the `HubClient` and stores
it in `AppState`, but the production router (`server/router.rs`) only installs
local authentication. `hub_auth_middleware` and `require_hub_auth_middleware`
(`hub/hub_auth.rs`) are defined and exported but never installed. The Hub
extractor (`hub/extractor.rs`) treats a missing `HubUserContext` as `None`, and
`MultiTenant` (`hub/multi_tenant.rs`) then keeps the raw key. Two clients with
different `x-hub-access-key` values never get validated and read and write the
same unscoped keys.

## What Changes

- Install Hub authentication in the production router when `hub.enabled` is
  true.
- Mode policy: in Hub mode every data request MUST carry a valid Hub access
  key. Local credentials alone are rejected on data routes. Health and metrics
  exemptions are listed explicitly.
- Carry the tenant in the phase16 `OperationContext`. Resource scoping fails
  closed: Hub mode without a tenant is an error, never the raw key.
- Scope MCP, UMICP and WebSocket operations by tenant.
- RESP3 and SynapRPC have no tenant authentication. In Hub mode the server
  refuses to start with either listener enabled and names the listener in the
  error.

## Impact

- Affected specs: hub (tenant isolation)
- Affected code: `crates/synap-server/src/main.rs`,
  `crates/synap-server/src/server/router.rs`, `crates/synap-server/src/hub/`,
  the MCP, UMICP and WebSocket handlers, config validation
- Dependencies: phase16 (`OperationContext`), phase17 (MCP/UMICP identity)
- Breaking change: YES. Hub deployments with RESP3 or SynapRPC enabled no
  longer start, and requests without a Hub key are rejected. Those
  deployments had no tenant isolation before this change.
- User benefit: tenants of a Hub deployment cannot read or overwrite each
  other's data.
