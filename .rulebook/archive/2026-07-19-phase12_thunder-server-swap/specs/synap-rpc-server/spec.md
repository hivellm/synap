# Spec: SynapRPC server on Thunder

## MODIFIED Requirements

### Requirement: RPC transport implementation
The SynapRPC listener SHALL be implemented on top of `thunder::server::spawn_listener`;
Synap MUST NOT maintain its own frame codec, accept loop or connection writer task.

#### Scenario: Listener serves the configured port
Given a Synap server configured with the SynapRPC listener enabled on port 15501
When the process starts
Then a Thunder listener is bound on 15501 and answers length-prefixed MessagePack frames

#### Scenario: No local codec remains
Given the `crates/synap-server` sources
When the RPC module is inspected
Then no length-prefix parsing, frame encoding or accept loop exists outside `thunder`

### Requirement: Protocol configuration
Synap SHALL declare its own `thunder::Config` in-repo with `Handshake::AuthCommand`,
`HelloStyle::NotUsed`, `PushPolicy::Enabled`, `ErrorConvention::Resp3Prefixes`,
`scheme = "synap"`, `default_port = 15501` and `max_frame_bytes = 512 MiB`.

#### Scenario: Frame cap preserved
Given a client sends a frame whose length prefix claims more than 512 MiB
When the server reads the prefix
Then the connection is rejected before the body is allocated

### Requirement: Command dispatch is unchanged
Every command the SynapRPC dispatch tree accepted before the swap SHALL return the
same value for the same arguments after it.

#### Scenario: KV round-trip
Given an authenticated RPC connection
When the client issues `SET k v` followed by `GET k`
Then the responses are `Ok(Str("OK"))` and a value decoding to `v`

#### Scenario: Unknown command
Given an authenticated RPC connection
When the client issues a command the dispatch tree does not know
Then the response is an `Err` whose message travels verbatim and the connection stays open

### Requirement: Authentication and ACL
Authentication SHALL be performed by `Dispatch::authenticate` over the existing
`UserManager`, and commands requiring admin privileges MUST still be refused with
`NOPERM` for non-admin principals when `require_auth` is enabled.

#### Scenario: Wrong password
Given a deployment with `require_auth = true`
When a client sends `AUTH user badpass`
Then the response is an error whose message starts with `WRONGPASS`

#### Scenario: Unauthenticated command
Given a deployment with `require_auth = true` and a connection that has not authenticated
When the client issues `GET k`
Then the response is an error whose message starts with `NOAUTH`

#### Scenario: Admin-only command as a non-admin
Given an authenticated non-admin principal on a `require_auth = true` deployment
When the client issues `FLUSHALL`
Then the response is an error whose message starts with `NOPERM`

#### Scenario: Open deployment
Given a deployment with `require_auth = false`
When a client issues `GET k` without authenticating
Then the command executes normally

### Requirement: Server push
The SUBSCRIBE flow SHALL deliver pub/sub messages as Thunder push frames
(`id == PUSH_ID`) through `Session::push_sender()`.

#### Scenario: Published message reaches a subscriber
Given a client that has issued SUBSCRIBE on a topic over RPC
When another client publishes to that topic
Then the subscriber receives a frame with `id == u32::MAX` carrying topic, payload, id and timestamp

## ADDED Requirements

### Requirement: Canonical `Bytes` encoding
The server SHALL emit `Value::Bytes` as MessagePack `bin` and MUST continue to
decode the legacy int-array form.

#### Scenario: Server emits bin
Given a stored binary value
When a client reads it over RPC
Then the response body encodes the value as a MessagePack `bin` object

#### Scenario: Legacy request accepted
Given a pre-Thunder SDK that encodes `Bytes` as an array of integers
When it sends a request carrying that form
Then the server decodes it as `Value::Bytes` and executes the command

### Requirement: Upstream gap reporting
Any Synap RPC capability that Thunder cannot express SHALL be filed as an issue on
`hivellm/thunder` before this task is archived; it MUST NOT be silently dropped or
patched around without a filed issue.

#### Scenario: Gap encountered
Given a Synap behavior with no Thunder equivalent
When the swap reaches that behavior
Then an issue exists on `hivellm/thunder` referencing the Synap call site, and the
in-repo mitigation is recorded in `.rulebook/knowledge/`
