## 1. Implementation (hivellm/synap-sdk-go)

- [x] 1.1 Add a `TransactionManager` with Multi/Exec/Discard/Watch/Unwatch and a generated client_id
- [x] 1.2 Map `transaction.*` to MULTI/EXEC/DISCARD/WATCH/UNWATCH in the command map
- [x] 1.3 Wrap writes carrying a client_id as TXQUEUE, refusing commands outside the server's queueable set
- [x] 1.4 Translate the EXEC result list and the control-command replies

## 2. Verification

- [x] 2.1 Unit tests pinning the wire names and the TXQUEUE wrapping
- [x] 2.2 S2S test proving a transaction is atomic on HTTP and on SynapRPC

## 3. Tail (docs + tests)

- [x] 3.1 Update or create documentation covering the implementation
- [x] 3.2 Write tests covering the new behavior
- [x] 3.3 Run tests and confirm they pass
