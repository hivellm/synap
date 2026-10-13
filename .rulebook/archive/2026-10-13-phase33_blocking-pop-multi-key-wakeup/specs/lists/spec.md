# Blocking Pops on Several Keys

## ADDED Requirements

### Requirement: Wake on any listed key
A client blocked in BLPOP, BRPOP, BZPOPMIN or BZPOPMAX on several keys MUST be
woken by data arriving on any of those keys, on every transport and whether or
not persistence is enabled.

#### Scenario: Push to the second key
Given a client blocked in BLPOP on `a` and `b` with a 10 second timeout
When another client runs RPUSH `b` `x`
Then the blocked client returns `[b, x]` well before the timeout

#### Scenario: Sorted set on the second key
Given a client blocked in BZPOPMIN on `z1` and `z2` with a 10 second timeout
When another client runs ZADD `z2` 1 `m`
Then the blocked client returns `[z2, m, 1]` well before the timeout

### Requirement: Argument order among ready keys
When a blocked client is woken, it SHALL pop from the first key, in argument
order, that holds data.

#### Scenario: Both keys pushed before the client runs
Given a client blocked in BLPOP on `a` and `b`
When `a` and `b` each receive one element before the client pops
Then the client returns the element of `a`
And `b` still holds its element

### Requirement: No lost wake-up
A push that lands after a blocking pop found every key empty and before it
started waiting MUST still wake it, and every element pushed while clients are
blocked MUST be delivered to one of them while any is still blocked.

#### Scenario: Racing pushes and blocked clients
Given 32 clients blocked in BLPOP on `q`
When 32 elements are pushed to `q` concurrently by 8 clients
Then every blocked client returns exactly one distinct element
And `q` is empty

### Requirement: First come, first served on one key
Clients blocked on the same key SHALL be woken in the order they blocked.

#### Scenario: One element for three blocked clients
Given clients A, B and C blocked in BLPOP on `q`, in that order
When one element is pushed to `q`
Then A returns it
And B and C stay blocked

### Requirement: Timeout unchanged
A blocking pop whose keys receive no data before its timeout MUST return nil
(RESP3 Null, SynapRPC Null), and a timeout of 0 MUST block until data arrives.

#### Scenario: No data before the timeout
Given a client running BLPOP `a` `b` 1 with `a` and `b` empty
When nothing is pushed for one second
Then the client receives nil
