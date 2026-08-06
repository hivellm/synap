## 1. Implementation (hivellm/synap-sdk-go)

- [ ] 1.1 Add a `TransactionManager` with Multi/Exec/Discard/Watch/Unwatch and a generated client_id
- [ ] 1.2 Map `transaction.*` to MULTI/EXEC/DISCARD/WATCH/UNWATCH in the command map
- [ ] 1.3 Wrap writes carrying a client_id as TXQUEUE, refusing commands outside the server's queueable set
- [ ] 1.4 Translate the EXEC result list and the control-command replies

## 2. Verification

- [ ] 2.1 Unit tests pinning the wire names and the TXQUEUE wrapping
- [ ] 2.2 S2S test proving a transaction is atomic on HTTP and on SynapRPC

## 3. Tail (docs + tests)

- [ ] 3.1 Update or create documentation covering the implementation
- [ ] 3.2 Write tests covering the new behavior
- [ ] 3.3 Run tests and confirm they pass
