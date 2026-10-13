## 1. Implementation
- [x] 1.1 Add a WAL format version, reading headerless files as the legacy format and failing startup on an unknown newer version
- [x] 1.2 Add a single-frame transaction record and write `log_transaction` batches as one record
- [x] 1.3 Replay a transaction record all-or-nothing
- [x] 1.4 Replicate a transaction as one unit that a replica applies whole or not at all
- [x] 1.5 Test truncation of a multi-operation transaction at every byte boundary, a transaction larger than the writer buffer, replica disconnect mid-transaction, and replay of a WAL fixture written by 1.3.2

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
