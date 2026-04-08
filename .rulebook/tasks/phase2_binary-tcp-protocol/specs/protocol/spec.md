# Binary TCP Protocol Specification

## ADDED Requirements

### Requirement: RESP3 Protocol Listener
The system MUST accept connections on a configurable TCP port using the Redis Serialization
Protocol version 3 (RESP3), enabling any Redis-compatible client to connect without changes.

#### Scenario: Redis client connects and runs SET/GET
Given resp3.enabled is true and a Redis client connects to resp3.port
When the client sends SET foo bar and then GET foo
Then the server MUST respond with +OK and $3\r\nbar\r\n respectively
And the response format MUST be valid RESP3

#### Scenario: Pipelining multiple commands
Given a client sends 10 commands without waiting for responses
When the server processes them
Then all 10 responses MUST be returned in order
And no command MUST be lost

#### Scenario: AUTH required when auth.enabled
Given auth.enabled is true
When a client connects and sends GET before AUTH
Then the server MUST respond with -NOAUTH Authentication required
And the GET MUST NOT be processed

### Requirement: SynapRPC Binary Protocol
The system MUST provide a native binary protocol on a dedicated TCP port using
length-prefixed MessagePack frames for minimum per-request overhead.

#### Scenario: Frame encoding
Given a Request with id=42, command="SET", args=["key", Bytes([1,2,3])]
When the frame is encoded
Then the first 4 bytes MUST be the LE u32 length of the MessagePack body
And the body MUST be valid MessagePack deserializable to the original Request

#### Scenario: Concurrent requests on one connection
Given a client sends 1000 requests with distinct id values without waiting for responses
When the server processes them (potentially out of order)
Then each Response MUST carry the same id as its corresponding Request
And all 1000 responses MUST be received by the client

#### Scenario: Connection pool in client
Given SynapRpcClient is configured with max_connections=8
When 100 concurrent requests are made
Then at most 8 TCP connections MUST be open simultaneously
And all requests MUST complete successfully

### Requirement: SDK Transport Auto-Negotiation
All SDKs MUST support a transport="auto" mode that connects via SynapRPC TCP when the
server supports it, and falls back to HTTP transparently when TCP is unavailable.

#### Scenario: Auto mode uses TCP when available
Given transport="auto" and synap_rpc.enabled is true on the server
When SynapClient connects
Then it MUST establish a TCP connection to synap_rpc.port
And all subsequent operations MUST use the TCP transport

#### Scenario: Auto mode falls back to HTTP
Given transport="auto" and the TCP port is not reachable
When SynapClient connects
Then it MUST fall back to HTTP transport
And the fallback MUST be transparent to the caller with no error raised

### Requirement: HTTP API Preserved
The existing HTTP REST API MUST remain fully functional after adding TCP protocols.
No existing HTTP endpoint MUST be removed or modified.

#### Scenario: HTTP and TCP coexist
Given both HTTP (port 15500) and SynapRPC (port 15501) are enabled
When clients connect to both simultaneously
Then both MUST operate independently and correctly
And shared state (KV store, queues, streams) MUST be consistent across both transports
