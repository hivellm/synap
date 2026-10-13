## 1. Implementation
- [ ] 1.1 Add a WAL format version, reading headerless files as the legacy format and failing startup on an unknown newer version
- [ ] 1.2 Add a single-frame transaction record and write `log_transaction` batches as one record
- [ ] 1.3 Replay a transaction record all-or-nothing
- [ ] 1.4 Replicate a transaction as one unit that a replica applies whole or not at all
- [ ] 1.5 Test truncation of a multi-operation transaction at every byte boundary, a transaction larger than the writer buffer, replica disconnect mid-transaction, and replay of a WAL fixture written by 1.3.2

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
