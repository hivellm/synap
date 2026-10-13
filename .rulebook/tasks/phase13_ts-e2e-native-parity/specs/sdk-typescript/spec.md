## MODIFIED Requirements

### Requirement: Listing parity across transports
`queue.listQueues()` and `stream.listRooms()` MUST return the same array of
names over HTTP, SynapRPC and RESP3.

#### Scenario: Listing queues over a native transport
Given a queue created through a SynapRPC client
When `listQueues()` is called on that client
Then the result is an array containing the queue name, not `undefined`

#### Scenario: Listing stream rooms over a native transport
Given a room created through a RESP3 client
When `listRooms()` is called on that client
Then the result is an array containing the room name, not `undefined`
