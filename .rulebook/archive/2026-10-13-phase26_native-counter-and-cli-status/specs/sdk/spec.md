# Counter Mapping and CLI Exit Status

## ADDED Requirements

### Requirement: Same counter arithmetic on every transport
Every SDK MUST produce the same stored value for an increment or decrement with
a given amount, whichever transport it uses.

#### Scenario: Increment by ten
Given counter `c` holding 0
When a client increments `c` by 10 over HTTP, then over SynapRPC, then over RESP3
Then `c` holds 30

#### Scenario: No amount
Given counter `c` holding 5
When a client increments `c` without an amount over RESP3
Then `c` holds 6

#### Scenario: Overflow
Given counter `c` holding the maximum 64-bit integer
When a client increments `c` by 1 over any transport
Then the client receives an error
And `c` is unchanged

### Requirement: Malformed CLI amounts rejected
The CLI MUST reject an amount that is not an integer without sending a command.

#### Scenario: Non-numeric amount
Given a running server
When a user runs `INCR c ten`
Then the CLI exits non-zero with an error naming the amount
And `c` is unchanged

### Requirement: PING exit status
The CLI MUST exit 0 from `PING` only when the server answered the ping, and
MUST exit non-zero with a diagnostic on stderr otherwise.

#### Scenario: Server down
Given no server listening on the target address
When a user runs the CLI with `PING`
Then the process exits non-zero
And stderr names the connection failure

#### Scenario: Denied credentials
Given a server requiring authentication
When a user runs `PING` with a wrong password
Then the process exits non-zero
