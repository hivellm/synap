# Stream room generation discriminator

## ADDED Requirements

### Requirement: Room creation stamp

Every stream room SHALL record the epoch-millisecond instant of its creation
and a generation id, and `RoomStats` SHALL carry both as `created_at` and
`generation`.

#### Scenario: Fresh room reports a creation stamp

Given a stream manager with no rooms
When a room `chat` is created and its stats are read
Then `created_at` is a non-zero epoch-millisecond value
And `generation` is a non-zero value

#### Scenario: Stats are stable while the room lives

Given a room `chat` that has been created
When its stats are read twice with a publish in between
Then `created_at` and `generation` are identical in both reads

### Requirement: Generation discriminates room recreation

The generation id SHALL be strictly greater for every room created after a
previous room creation in the same process, so a recreated room never reports a
generation a consumer has already observed.

#### Scenario: Recreated room reports a new generation

Given a room `chat` whose stats report generation `G`
When the room is deleted and created again under the same name
Then its stats report a generation strictly greater than `G`

#### Scenario: Distinct rooms report distinct generations

Given two rooms `a` and `b` created in sequence
When both report their stats
Then the generation of `b` is strictly greater than the generation of `a`

### Requirement: Generation survives process restarts

The generation source SHALL be seeded from the wall clock so a room created by
a new server process never reports a generation already emitted by a previous
process.

#### Scenario: Generation tracks wall-clock milliseconds

Given a process that has created no rooms
When the first room is created
Then its generation is greater than or equal to the epoch-millisecond instant
observed just before the creation

### Requirement: Native transports expose the discriminator

The RESP3 `SSTATS` and SynapRPC `SSTATS` replies SHALL include `created_at` and
`generation` entries, so consumers on the native transports detect wipes the
same way HTTP consumers do.

#### Scenario: RESP3 SSTATS carries the discriminator

Given a stream room `chat`
When a RESP3 client issues `SSTATS chat`
Then the reply map contains `created_at` and `generation` integer entries

#### Scenario: SynapRPC SSTATS carries the discriminator

Given a stream room `chat`
When a SynapRPC client issues `SSTATS chat`
Then the reply map contains `created_at` and `generation` integer entries
