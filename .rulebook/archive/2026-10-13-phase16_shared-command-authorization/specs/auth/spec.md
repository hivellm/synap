# Command Authorization

## ADDED Requirements

### Requirement: One authorization contract for every transport
When authentication is enabled, the server MUST authorize every command
received on the REST command envelope, RESP3 and SynapRPC against the caller's
effective permissions for every resource the command touches, using the same
resource names as the dedicated REST handlers.

#### Scenario: Envelope flush denied
Given authentication is required and a user whose only role is read-only
When the user posts `{"command":"kv.flushall","request_id":"r1","payload":{}}` to `/api/v1/command`
Then the server answers with a permission error
And every key is still present

#### Scenario: Same denial on every transport
Given a user granted `read` on `kv:*` only
When the user sends SET on key `a` through REST, the envelope, RESP3 and SynapRPC
Then all four requests are denied
And key `a` is unchanged

#### Scenario: Authorized operation succeeds
Given a user granted `write` on `kv:orders:*`
When the user sends SET on `orders:1` through RESP3
Then the write succeeds

### Requirement: Multi-resource commands
The server MUST authorize every source and destination resource of a command,
every key of a multi-key command, every declared script key, and every command
a script issues, against the caller's context.

#### Scenario: Destination outside the grant
Given a user granted `read` and `write` on `set:a*` only
When the user sends SUNIONSTORE with destination `b` and sources `a1`, `a2`
Then the command is denied
And `b` does not exist

#### Scenario: Script reaches a forbidden key
Given a user granted `write` on `kv:x` only
When the user runs a script whose body writes `kv:y`
Then the write to `y` is denied
And `y` is unchanged

### Requirement: Transactions are all-or-nothing under authorization
The server MUST abort EXEC without applying any command when any command
queued in the transaction was denied.

#### Scenario: Denied queued command aborts the transaction
Given a user granted `write` on `kv:a` only
When the user queues SET `a` and SET `b` and sends EXEC
Then EXEC fails
And neither `a` nor `b` changes

### Requirement: Fail closed on unclassified commands
When authentication is enabled, the server MUST deny a command that has no
access classification.

#### Scenario: Every dispatcher command is classified
Given the full list of envelope, RESP3 and SynapRPC command names
When the classification test runs
Then every command has a classification
And the test fails naming any command that does not

### Requirement: Explicit unrestricted mode
When authentication is disabled, the server SHALL allow every command, and
this MUST be decided by configuration, never by the absence of a context.

#### Scenario: Auth disabled
Given authentication is disabled
When an anonymous client sends FLUSHALL over RESP3
Then the command runs
