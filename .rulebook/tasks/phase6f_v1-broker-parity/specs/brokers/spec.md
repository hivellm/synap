## MODIFIED Requirements

### Requirement: Streams do not silently drop unread events
Stream retention MUST NOT discard events that no consumer has read without either persisting
them to disk or emitting an explicit drop/lag signal.

#### Scenario: Slow consumer does not silently lose data
Given a producer outrunning a consumer past the in-memory buffer size
When retention would drop the oldest unread events
Then those events are either spilled to disk or the drop is surfaced as an explicit signal,
not lost silently

### Requirement: Constant-time stream seek
Consuming from an offset MUST be O(1) with respect to buffer size, using the contiguous-offset
index rather than a linear scan.

#### Scenario: Seek in a large buffer
Given a room with a large event buffer
When a consumer requests events from a recent offset
Then the lookup does not scan the entire buffer

### Requirement: Fair multi-consumer queue dispatch
Queues MUST support multiple consumers with a per-consumer prefetch limit and fair dispatch,
and MUST report the actual consumer count.

#### Scenario: Two consumers share a queue fairly
Given two consumers with prefetch=1 on the same queue
When ten messages are published
Then the messages are distributed across both consumers and stats report two consumers

### Requirement: ACK-deadline expiry without global stall
Expired-pending detection MUST NOT hold a global lock across all queues nor scan every pending
entry on a fixed interval.

#### Scenario: Deadline expiry scales
Given many queues with large pending sets
When ACK deadlines expire
Then only expired messages are processed, without blocking operations on unrelated queues
