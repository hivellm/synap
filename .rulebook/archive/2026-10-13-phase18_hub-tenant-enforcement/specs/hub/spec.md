# Hub Tenant Isolation

## ADDED Requirements

### Requirement: Hub authentication on the production router
When `hub.enabled` is true, the server MUST validate the Hub access key of
every data request against the Hub API before handling it.

#### Scenario: Missing key
Given a server started with Hub enabled through the production startup path
When a client calls a KV route without `x-hub-access-key`
Then the server answers 401

#### Scenario: Invalid key
Given a server started with Hub enabled and a mock Hub API that rejects key `bad`
When a client calls a KV route with `x-hub-access-key: bad`
Then the server answers 401

### Requirement: Tenant isolation
In Hub mode, the server MUST scope every resource by the caller's tenant and
MUST fail the request instead of using an unscoped name when no tenant is
present.

#### Scenario: Same logical key, two tenants
Given two valid Hub keys belonging to tenants A and B
When tenant A sets `config` to `a` and tenant B sets `config` to `b`
Then tenant A reads `a`
And tenant B reads `b`

#### Scenario: Isolation on every enabled surface
Given tenants A and B on a Hub-mode server
When each writes the same logical key through REST, the envelope, MCP, UMICP and WebSocket
Then neither tenant can read or overwrite the other's value

### Requirement: Unsupported listeners refused
The server MUST refuse to start in Hub mode while a listener without tenant
authentication (RESP3, SynapRPC) is enabled.

#### Scenario: RESP3 enabled in Hub mode
Given `hub.enabled: true` and `resp3.enabled: true`
When the server starts
Then startup fails with an error naming the RESP3 listener
