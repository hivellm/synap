# MCP and UMICP Tool Authorization

## ADDED Requirements

### Requirement: Explicit request identity
The server MUST pass the authenticated identity of each MCP and UMICP request
to tool dispatch explicitly, and MUST NOT rely on thread-local or other
ambient storage to identify the caller.

#### Scenario: Interleaved sessions keep their own permissions
Given an admin MCP session and a read-only MCP session on a multi-threaded runtime
When both call `synap_kv_set` repeatedly and concurrently
Then every admin call succeeds
And every read-only call is denied

#### Scenario: Read-only UMICP caller
Given authentication is required and a read-only user
When the user sends a UMICP envelope invoking `synap_kv_set`
Then the call is denied
And the key is unchanged

### Requirement: Session binding
The server MUST bind an MCP session to the principal that initialized it and
MUST reject requests on that session carrying a different principal.

#### Scenario: Session reused with other credentials
Given an MCP session initialized by user `alice`
When a request on that session id authenticates as `bob`
Then the request is rejected

### Requirement: Fail closed without context
When authentication is enabled, the server MUST deny a tool call that has no
identity context.

#### Scenario: Missing context
Given authentication is enabled
When a tool call reaches dispatch without an identity context
Then the call is denied
