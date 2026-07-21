# Streams — consume out-of-range signalling

## ADDED Requirements

### Requirement: Explicit out-of-range on evicted offsets
The stream `consume` operation SHALL return an explicit out-of-range error when
the requested `from_offset` is below the room's earliest retained offset
(`min_offset`) after eviction, instead of silently returning events from the
earliest retained offset.

#### Scenario: Consume from an evicted offset
Given a stream room whose retention has evicted all events below offset 500
When a consumer calls consume with `from_offset = 100`
Then the operation MUST return `OffsetOutOfRange { requested: 100, earliest: 500 }`
And the server MUST respond with HTTP 400 carrying the earliest available offset

#### Scenario: Consume from a retained offset is unaffected
Given a stream room retaining offsets 500 through 900
When a consumer calls consume with `from_offset = 600` and `limit = 10`
Then the operation MUST return events 600..=609 with no error

#### Scenario: Consume from a fresh room is unaffected
Given a stream room that has never evicted (`min_offset == 0`)
When a consumer calls consume with `from_offset = 0`
Then the operation MUST return the available events with no error
