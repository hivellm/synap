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
the events older than that many seconds, and the room's earliest offset and
statistics MUST reflect what it still holds, also when no event is left.

#### Scenario: Every event expired
Given a stream room with `retention_secs = 60` whose events are all older than a minute
When compaction runs and an event is published afterwards
Then the room reports one message, with the new event's offset as its earliest offset
And a consumer reading from that offset receives the new event
And a consumer reading from an expired offset gets an out-of-range error
