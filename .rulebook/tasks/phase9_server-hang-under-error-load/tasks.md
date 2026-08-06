## 1. Reproduce

- [ ] 1.1 Replay the PHP parity harness concurrently across all three transports against a debug build
- [ ] 1.2 Include the malformed queue publishes that preceded the observed hang
- [ ] 1.3 Loop until it reproduces, recording the command sequence that did it

## 2. Diagnose

- [ ] 2.1 Dump task and thread state while hung (tokio-console or a thread dump)
- [ ] 2.2 Identify the lock and the holder
- [ ] 2.3 Audit `parking_lot` guards held across `.await` in the dispatch path and the periodic snapshot task

## 3. Fix

- [ ] 3.1 Release the offending lock before awaiting, or move the shared state behind an async-aware lock
- [ ] 3.2 Regression test: hammer every transport with valid and malformed commands, assert the server still answers

## 4. Tail (docs + tests)

- [ ] 4.1 Update or create documentation covering the implementation
- [ ] 4.2 Write tests covering the new behavior
- [ ] 4.3 Run tests and confirm they pass
