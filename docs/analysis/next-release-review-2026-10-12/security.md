# Security and transport authorization review

> **Historical analysis.** Describes Synap 1.3.2: reviewed 2026-10-12 against
> `5b1efd1` plus the uncommitted 1.3.2 update, committed in `bfecd0c`
> (2026-10-13). The 1.4.0 release was built from this review (see
> `CHANGELOG.md`, section `[1.4.0]`); its file and line references point at the
> 1.3.2 tree. The code has moved since: paths, line numbers and findings
> describe that revision, not the current tree. Current behavior:
> [1.4.0 guarantees](../../operations/guarantees.md). Current design:
> [persistence](../../specs/PERSISTENCE.md),
> [transports](../../protocol/transports.md),
> [KV store internals](../../internals/kv-store.md) and the
> [documentation index](../../INDEX.md).

Reviewed: 2026-10-12. Baseline: current 1.3.2 working tree, including external dependency/version updates and MCP ServerConfig migration. Existing changes were preserved.

Evidence below is static source inspection, revalidated after those updates. No exploit was executed, no server was launched, and no cargo command was run for this report. Scenarios describe behavior implied by the inspected paths; acceptance criteria are proposed regression tests, not completed validation.

## Shared remediation direction

Transport authentication, resource/action authorization, and tenant identity should feed one shared authorized command-dispatch contract. Avoid independent ACL patches that drift between REST, command envelopes, MCP, UMICP, RESP3, and Thunder-backed SynapRPC. Each surface must have its own end-to-end denial and success regression tests, including WebSocket/subscription operations where supported. Preserve deliberately unrestricted standalone deployments with authentication disabled; do not interpret missing context as that deployment mode.

## S-01 - API key creation permits permission escalation

Priority: P1. Confidence: high, static call-path evidence.

Evidence: `crates/synap-server/src/server/auth_handlers.rs:141` accepts caller-supplied permissions and creates a key without requiring an authenticated issuer or checking that requested grants are a subset of issuer privileges. `crates/synap-server/src/auth/middleware.rs:166` subsequently trusts the key's stored permissions.

Scenario: a read-only user posts `{"name":"review","permissions":[{"resource":"*","action":"all"}]}` to `/auth/keys`. The returned bearer key can authorize previously forbidden REST writes and deletes. Anonymous creation is also reachable when local authentication is enabled but optional. The key remains `is_admin=false`; this finding does not claim access to user-management handlers that check that flag directly.

Recommended fix: require an authenticated issuer when authentication is enabled and constrain grants to issuer authority, including wildcard/resource semantics. Define delegation rules for API-key issuers and ensure a restricted key cannot mint an unrestricted successor.

Acceptance criteria:

- Read-only and resource-scoped callers cannot mint broader action or resource grants.
- Anonymous creation is rejected when authentication is enabled.
- A permitted narrower grant remains usable; rejected requests create no stored key.
- Invalid permission actions are rejected rather than silently interpreted as read access.

## S-02 - Command-envelope HTTP endpoint bypasses operation ACL

Priority: P1. Confidence: high, static call-path evidence.

Evidence: `crates/synap-server/src/server/handlers/mod.rs:298` extracts state and JSON without AuthContext. Dispatch at `crates/synap-server/src/server/handlers/mod.rs:329` exposes `kv.flushall`; `crates/synap-server/src/server/handlers/kv_cmd.rs:780` directly calls `store.flushall()` without an authorization check. Authentication middleware establishes identity but does not authorize individual commands.

Scenario: with required authentication, a no-role or read-only user posts `{"command":"kv.flushall","request_id":"review","payload":{}}` to `/api/v1/command`. This path reaches a whole-KV deletion despite the user's lack of deletion/admin permission. Ordinary key reads and writes also bypass the resource checks present in dedicated REST handlers.

Recommended fix: require the shared authorized command context at envelope dispatch; authorize every source/destination resource and protect global operations with an explicit administrative policy.

Acceptance criteria:

- Restricted identities receive equivalent denials through dedicated REST and command-envelope endpoints.
- Denied global operations leave all keys unchanged; authorized administrative operations succeed.
- Multi-key operations, scripting, transactions, aliases, and source/destination commands cannot evade per-resource checks.

## S-03 - Production Hub mode never installs Hub authentication

Priority: P1. Confidence: high, static startup and router evidence.

Evidence: `crates/synap-server/src/main.rs:759` initializes HubClient and `crates/synap-server/src/main.rs:838` stores it in AppState. Startup constructs the ordinary router at `crates/synap-server/src/main.rs:885` and serves it at `crates/synap-server/src/main.rs:908`. `crates/synap-server/src/server/router.rs:459` installs local authentication, not Hub authentication. A source-wide search finds `hub_auth_middleware` and `require_hub_auth_middleware` only at their definitions and exports. `crates/synap-server/src/hub/extractor.rs:24` treats missing HubUserContext as None, and `crates/synap-server/src/hub/multi_tenant.rs:48` consequently preserves the raw KV key.

Scenario: enable Hub integration with local authentication disabled. Requests carrying distinct `x-hub-access-key` values never undergo Hub validation and address the same unscoped keys. Enabling local required authentication instead rejects Hub-only clients or permits locally authenticated calls without tenant scope. Initializing a HubClient alone does not establish tenant isolation.

Recommended fix: install the appropriate Hub middleware in production startup/router wiring; require tenant context when Hub mode demands it; explicitly define hybrid local/Hub behavior. Reject unsupported binary protocol configurations in Hub mode until their tenant authentication and scoping are implemented.

Acceptance criteria:

- Tests construct the production router with Hub enabled and a mock Hub API, without manually injecting tenant extensions.
- Missing and invalid Hub credentials fail according to the explicit mode policy.
- Two valid tenants using the same logical key cannot read or overwrite one another's data.
- Every enabled protocol and subscription surface passes isolation tests or is explicitly unavailable in Hub mode.

## S-04 - MCP authorization context is unsafe across async work; UMICP drops it

Priority: P1. Confidence: high for missing-context and UMICP paths; MCP concurrency behavior requires transport regression reproduction.

Evidence: `crates/synap-server/src/auth/mcp_context.rs:8` stores identity in thread-local storage. `crates/synap-server/src/server/router.rs:621` sets it before `service.call(req).await` at line 625, then clears it after the await. Async tasks can interleave or move across worker threads, so this storage does not identify a request/session reliably. `crates/synap-server/src/server/mcp_handlers.rs:24` allows access when context is absent. Separately, `crates/synap-server/src/server/umicp/transport.rs:15` does not preserve request AuthContext, and `crates/synap-server/src/server/umicp/handlers.rs:43` calls the same MCP dispatcher directly.

Scenario: a restricted authenticated UMICP caller invokes `synap_kv_set`. Its identity is not passed to tool authorization, whose missing-context branch permits the write. Concurrent MCP requests can likewise lose or observe unrelated thread-local identity; exact scheduling outcomes have not been dynamically reproduced here.

Recommended fix: carry explicit request/session auth and tenant context into tool dispatch and fail closed on missing context when authentication is enabled. A task-local replacement alone is insufficient unless identity is deliberately propagated into transport/session tasks spawned by the MCP library.

Acceptance criteria:

- Read-only UMICP callers cannot invoke write/delete tools, while authorized callers succeed.
- Interleaved administrative and restricted MCP sessions retain their own permissions across yields and worker-thread changes.
- Missing context cannot authorize an operation in authenticated mode.
- Session creation, reuse, and tool dispatch bind identity consistently; standalone auth-disabled behavior remains intentional and tested.

## S-05 - Binary listeners enforce admin commands but ignore resource/action ACL

Priority: P1. Confidence: high, static dispatcher evidence.

Evidence: `crates/synap-server/src/protocol/resp3/server.rs:262` and `crates/synap-server/src/protocol/synap_rpc/server.rs:110` only check `command_requires_admin`. The command list at `crates/synap-server/src/auth/permissions.rs:222` covers global/admin operations. Subsequent dispatch passes no principal/permission context to ordinary commands.

Scenario: authenticate a no-role or read-only user on either enabled listener while authentication is required. GET, SET, and DEL on arbitrary keys do not undergo resource/action ACL evaluation, even when dedicated REST endpoints would deny the same request. Existing admin-command protection does not address this case.

Recommended fix: use the shared authorized command-dispatch contract for RESP3 and Thunder-backed SynapRPC, with all source/destination keys covered. Keep global admin checks and explicitly define authenticated versus anonymous behavior when authentication is optional.

Acceptance criteria:

- Both listeners run a no-role/read-only/resource-scoped/admin authorization matrix.
- Forbidden GET, SET, DEL, multi-key, transaction, script, and subscription operations fail without side effects.
- Authorized operations succeed and global administrative commands retain their stronger checks.

## S-06 - Configured rate limiting is not enforced

Priority: P2. Confidence: high, explicit implementation evidence.

Evidence: `crates/synap-server/src/server/router.rs:559` only logs that configured rate limiting is not active and needs middleware integration. `crates/synap-server/src/main.rs:897` nevertheless logs that rate limiting is enabled.

Scenario: an operator enables request/burst limits expecting authentication/API traffic to be bounded. The router accepts traffic without that enforcement. This also leaves expensive bcrypt authentication work outside the promised limit.

Recommended fix: integrate the limiter with defined client-IP/identity semantics and authentication ordering, or reject enabled configuration until supported. Startup messages must describe actual enforcement.

Acceptance criteria:

- Burst and sustained traffic receive expected 429 responses with documented retry behavior.
- Capacity recovers after refill; independent clients follow documented fairness policy.
- Authentication attempts are protected and health-probe exemptions are explicit.
- Enabled/disabled startup logs agree with observed behavior.

## S-07 - UMICP buffers bodies without an application bound

Priority: P2. Confidence: high for unbounded consumption; memory-exhaustion impact not load-tested.

Evidence: `crates/synap-server/src/server/umicp/transport.rs:19` calls `axum::body::to_bytes(request.into_body(), usize::MAX)`. The router has no RequestBodyLimit layer. This raw Request path does not use the normal JSON extractor's bounded body consumption.

Scenario: a client able to reach UMICP after applicable authentication supplies an oversized or chunked request. The server accumulates the complete body before envelope parsing, allowing memory use far beyond ordinary request sizes. Concurrent requests compound this pressure; a reverse-proxy limit is not an application guarantee.

Recommended fix: apply an explicit configurable request-body bound to the actual UMICP stream, reject over-limit requests with 413, and avoid consuming the remainder before rejection. Keep envelope validation within the bound.

Acceptance criteria:

- Valid bodies at the supported limit are accepted; bodies over it receive 413.
- Both Content-Length and chunked bodies obey the limit, including misleading/missing length cases handled by the HTTP stack.
- Oversized malformed envelopes fail before unbounded buffering/parsing; ordinary discovery and valid tool calls remain functional.

## Excluded stale finding

New passwords use bcrypt and legacy SHA-512 verification supports migration in `crates/synap-server/src/auth/user.rs:66`. Do not reopen a claim that current new passwords use unsalted SHA-512 solely because older project documentation says so.
