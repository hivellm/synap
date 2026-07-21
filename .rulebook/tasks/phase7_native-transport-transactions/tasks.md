## 1. Implementation
- [ ] 1.1 Decide the native-wire transaction queuing design (connection-scoped Redis-style vs. explicit client_id carriage) and record it as an ADR
- [ ] 1.2 Implement queuing in the SynapRPC dispatcher + TransactionManager
- [ ] 1.3 Implement queuing in the RESP3 dispatcher (Redis MULTI semantics)
- [ ] 1.4 Update the TS SDK command map (remove the silent-bypass path), the Python SDK (drop the client_id refusal guard), and the C# SDK mapper
- [ ] 1.5 Add MULTI → queued write → EXEC/DISCARD parity S2S tests on all three transports in each SDK

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (transactions doc: per-transport semantics; CHANGELOG)
- [ ] 2.2 Write tests covering the new behavior (server dispatch tests + SDK parity suites)
- [ ] 2.3 Run tests and confirm they pass (full workspace + all SDK suites incl. S2S)
