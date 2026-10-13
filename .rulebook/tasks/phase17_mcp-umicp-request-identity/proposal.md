# Proposal: phase17_mcp-umicp-request-identity

Source: docs/analysis/next-release-review-2026-10-12/ (finding S-04, priority P1)
Target release: 1.4.0

## Why

MCP tool authorization reads the caller from a thread-local
(`crates/synap-server/src/auth/mcp_context.rs`). The `/mcp` route in
`server/router.rs` sets it before `service.call(req).await` and clears it after,
but rmcp 3.5's streamable HTTP server runs session and tool work in tasks it
spawns itself (`tokio::spawn` in `rmcp/src/transport/streamable_http_server/tower.rs`).
The tool handler therefore usually runs on a task that never saw the
thread-local, or sees one left by another request on the same worker.
`check_mcp_permission` in `server/mcp_handlers.rs` allows the call when the
context is absent, so in authenticated mode MCP permissions are not reliably
enforced at all. UMICP (`server/umicp/transport.rs`, `umicp/handlers.rs`) never
passes the request's `AuthContext` and calls the same dispatcher, which takes
the same allow branch.

## What Changes

- Delete the thread-local and pass the phase16 `OperationContext` explicitly to
  `handle_mcp_tool` and every tool handler.
- Carry the authenticated identity from the `/mcp` HTTP request into rmcp's
  per-request context, bind it to the MCP session at initialization, and reject
  a session reused under a different principal.
- Pass the UMICP request's `AuthContext` into tool dispatch.
- In authenticated mode a missing context denies the call. Auth-disabled
  access stays explicit.
- Tool authorization uses the phase16 classifier and resource names, so a tool
  and its REST equivalent agree.

## Impact

- Affected specs: mcp (tool authorization), umicp
- Affected code: `crates/synap-server/src/auth/mcp_context.rs` (removed),
  `crates/synap-server/src/server/router.rs`,
  `crates/synap-server/src/server/mcp_handlers.rs`,
  `crates/synap-server/src/server/umicp/`
- Dependencies: phase16 (`OperationContext` and classifier)
- Breaking change: YES for authenticated MCP/UMICP clients that currently get
  through without matching permissions.
- User benefit: an MCP or UMICP caller gets exactly the permissions of its
  credentials, under concurrency too.
