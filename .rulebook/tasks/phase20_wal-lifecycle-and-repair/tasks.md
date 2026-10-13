## 1. Implementation
- [ ] 1.1 Flush and fsync the WAL on an independent interval in Periodic mode, including while idle
- [ ] 1.2 Hand buffered bytes to the OS after each group in Never mode, without fsync
- [ ] 1.3 Add an awaited flush barrier, back `PersistenceLayer::flush` with it, and drain the WAL on graceful shutdown
- [ ] 1.4 Fail every acknowledgement in a group whose flush or fsync fails, and poison the writer so every subsequent append fails until restart
- [ ] 1.5 Report a poisoned WAL in the health endpoint and logs
- [ ] 1.6 Return an error from the REST, admin, MCP, RESP3 and SynapRPC EXEC paths when the transaction could not be logged
- [ ] 1.7 Verify checksums and bound frame length while scanning, in both the async and sync scanners
- [ ] 1.8 Truncate a torn tail to the last valid frame before appending, and fail startup on interior corruption with the byte position
- [ ] 1.9 Add a writer seam that injects flush and fsync failures, and fault-injection tests for the idle interval, shutdown, injected failures, and every torn-tail shape across two restarts

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
