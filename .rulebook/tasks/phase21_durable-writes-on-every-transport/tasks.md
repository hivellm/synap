## 1. Implementation
- [ ] 1.1 List every mutating command on REST, the envelope, RESP3, SynapRPC and MCP with how each is persisted today
- [ ] 1.2 Add one mutation path that records a command's effect through the persistence layer before acknowledging it, shared by every transport
- [ ] 1.3 Route every RESP3 mutating command through the shared path
- [ ] 1.4 Route every SynapRPC mutating command through the shared path
- [ ] 1.5 Return the resulting value and expiry from APPEND and SETRANGE under the key's shard lock, and log that value on REST and the envelope instead of an empty one
- [ ] 1.6 Route the remaining REST, envelope and MCP writes through the shared path
- [ ] 1.7 Add a parity test that runs every mutating command on each transport, restarts from the WAL, compares the recovered state, and checks a replica holds the same state

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
