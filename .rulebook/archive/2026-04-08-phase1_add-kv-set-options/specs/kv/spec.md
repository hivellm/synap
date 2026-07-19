# KV Store Specification — SET Options

## ADDED Requirements

### Requirement: Conditional SET (NX / XX)
The system MUST support conditional SET operations that check key existence atomically
under the same shard write lock, with no TOCTOU race.

#### Scenario: NX — set only if absent
Given a key does not exist
When SET is called with `if_absent: true`
Then the key MUST be created and the response indicates success

#### Scenario: NX — skip if present
Given a key already exists
When SET is called with `if_absent: true`
Then the key MUST NOT be overwritten and the response indicates the condition was not met

#### Scenario: XX — set only if present
Given a key already exists
When SET is called with `if_present: true`
Then the key MUST be updated

#### Scenario: XX — skip if absent
Given a key does not exist
When SET is called with `if_present: true`
Then nothing MUST be written and the response indicates the condition was not met

#### Scenario: NX is atomic under contention
Given 100 concurrent goroutines/tasks all call SET with the same key and `if_absent: true`
When all calls complete
Then exactly 1 MUST have succeeded and 99 MUST have received a condition-not-met response

### Requirement: GET Option (Return Previous Value)
The system MUST support returning the previous value of a key atomically as part of a SET operation.

#### Scenario: GET returns old value
Given a key exists with value "old"
When SET is called with new value "new" and `return_old: true`
Then the response MUST include "old" as the previous value
And the key MUST now hold "new"

#### Scenario: GET on new key returns nil
Given a key does not exist
When SET is called with `return_old: true`
Then the previous value in the response MUST be absent/nil

### Requirement: KEEPTTL Option
The system MUST preserve the existing TTL when KEEPTTL is set, without resetting or removing it.

#### Scenario: TTL preserved on update
Given a key exists with a TTL of 60 seconds
When SET is called with a new value and `keep_ttl: true`
Then the key MUST hold the new value
And the TTL MUST remain at approximately 60 seconds (not reset)

### Requirement: Millisecond-Precision Expiry
The system MUST support TTL specified in milliseconds (PX) and as absolute Unix timestamps
in both seconds (EXAT) and milliseconds (PXAT).

#### Scenario: PX sets millisecond TTL
Given a SET request with `expiry: Milliseconds(500)`
When the SET is executed
Then the key MUST expire approximately 500ms after insertion

#### Scenario: PXAT sets absolute millisecond expiry
Given a SET request with `expiry: UnixMilliseconds(T)` where T is a future timestamp
When the current time passes T
Then the key MUST no longer be accessible
