# Streams — retention of 0 keeps events

## ADDED Requirements

### Requirement: Zero retention means no time-based retention
A stream room whose `retention_secs` is `0` MUST keep its events regardless of
their age. Compaction SHALL remove no event from such a room, and its buffer
caps (`max_buffer_size`, `max_unread_buffer_size`) MUST still apply.

#### Scenario: Old events survive compaction
Given a stream room with `retention_secs = 0` holding events published long ago
When compaction runs
Then every event is still in the room at its offset

#### Scenario: Buffer cap still applies
Given a stream room with `retention_secs = 0` and `max_buffer_size = 4`
When six events are published or restored
Then only the newest four are kept

### Requirement: Positive retention expires old events consistently
A stream room whose `retention_secs` is positive MUST remove, on compaction,
its oldest events while they are older than that many seconds, and SHALL stop
at the first event that is not, so the offsets it keeps stay contiguous. An
event whose timestamp is later than those of the events after it (the wall
clock was stepped back) therefore keeps them until it is that old itself,
while the buffer caps still bound the room. The room's earliest offset and
statistics MUST reflect what it still holds, also when no event is left.

#### Scenario: Old events expire
Given a stream room with `retention_secs = 60` whose two oldest events are older than a minute and whose newest is not
When compaction runs
Then the two oldest events are removed and the room's earliest offset is the newest event's offset
And a consumer reading from an expired offset gets an out-of-range error

#### Scenario: Every event expired
Given a stream room with `retention_secs = 60` whose events are all older than a minute
When compaction runs and an event is published afterwards
Then the room reports one message, with the new event's offset as its earliest offset
And a consumer reading from that offset receives the new event
And a consumer reading from an expired offset gets an out-of-range error

#### Scenario: Clock stepped back
Given a stream room with `retention_secs = 60` whose oldest event is older than a minute, followed by a recent event, an event older than a minute and a recent event
When compaction runs
Then only the oldest event is removed, and the room keeps the other three at contiguous offsets
When the recent event at the front is older than a minute as well and compaction runs again
Then it and the event older than a minute behind it are removed
