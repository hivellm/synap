## 1. Implementation
- [ ] 1.1 Add `OperationContext` and build it from the REST `AuthContext`, the RESP3 `AUTH` user and the Thunder session principal
- [ ] 1.2 Add the command access classifier mapping each command and its arguments to `(resource, action)` pairs, with an explicit policy for keyspace-wide commands
- [ ] 1.3 Add a test that fails when any envelope, RESP3 or SynapRPC command has no classification
- [ ] 1.4 Authorize `/api/v1/command` before dispatch
- [ ] 1.5 Authorize every RESP3 command, keeping the admin-command gate
- [ ] 1.6 Authorize every SynapRPC command, keeping the admin-command gate
- [ ] 1.7 Authorize transaction-queued commands (abort EXEC on any denial) and commands issued from scripts against the caller's context
- [ ] 1.8 Authorize subscribe and watch operations on their channels and keys
- [ ] 1.9 Run the no-role, read-only, resource-scoped and admin matrix on REST, envelope, RESP3 and SynapRPC

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
