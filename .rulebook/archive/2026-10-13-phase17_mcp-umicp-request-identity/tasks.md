## 1. Implementation
- [x] 1.1 Confirm how rmcp 3.5's streamable HTTP server exposes the originating HTTP request and its extensions to tool handlers
- [x] 1.2 Replace the thread-local `mcp_context` with an explicit `OperationContext` parameter on `handle_mcp_tool` and every tool handler
- [x] 1.3 Carry the `/mcp` request identity into tool dispatch, bind it to the MCP session, and reject a session reused under a different principal
- [x] 1.4 Pass the UMICP request's `AuthContext` into tool dispatch
- [x] 1.5 Deny tool calls without a context when authentication is enabled; keep auth-disabled access explicit
- [x] 1.6 Authorize tools through the phase16 classifier and resource names
- [x] 1.7 Test interleaved admin and restricted MCP sessions on a multi-threaded runtime, and read-only UMICP callers against write and delete tools

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
