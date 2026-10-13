# Write Durability Across Transports

## ADDED Requirements

### Requirement: Every acknowledged write is persisted and replicated
When persistence is enabled, the server MUST record the effect of every
mutating command through the persistence layer before acknowledging it,
whichever transport carried the command, and MUST send the same effect to
replicas.

#### Scenario: RESP3 write survives restart
Given persistence enabled with Always mode
When a client sends `SET k v` over RESP3 and the server restarts
Then `GET k` returns `v`

#### Scenario: SynapRPC write reaches a replica
Given a master with one connected replica
When a client sends `HSET h f v` over SynapRPC
Then the replica's `HGET h f` returns `v`

#### Scenario: Transport parity
Given the list of mutating commands
When each runs on REST, the envelope, RESP3, SynapRPC and MCP where supported, and the server restarts
Then the recovered state equals the state before the restart for every transport

### Requirement: Exact values for derived writes
The server MUST record APPEND and SETRANGE as the exact resulting value and its
current expiry, read under the same shard lock as the mutation.

#### Scenario: APPEND keeps bytes and expiry
Given key `k` holding `ab` with 300 seconds to live
When a client appends `cd` over REST and the server restarts
Then `k` holds `abcd`
And `k` still has its remaining time to live

#### Scenario: SETRANGE with zero padding
Given key `k` does not exist
When a client sends SETRANGE `k` 3 `x` over the envelope and the server restarts
Then `k` holds three zero bytes followed by `x`
