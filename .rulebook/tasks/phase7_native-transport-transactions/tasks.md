## 1. Implementation
- [x] 1.1 Decide the native-wire transaction queuing design (connection-scoped Redis-style vs. explicit client_id carriage) and record it as an ADR — ADR 005: `TXQUEUE <client_id> <CMD> <args...>` wrapper, client_id-keyed (cross-transport transactions for free; connection-scoped remains a possible additive layer)
- [x] 1.2 Implement queuing in the SynapRPC dispatcher + TransactionManager — TXQUEUE parses the inner command into the existing TransactionCommand enum and queues via queue_command_if_transaction; explicit errors for no-open-MULTI and unqueueable commands
- [x] 1.3 Implement queuing in the RESP3 dispatcher — same TXQUEUE command, replies QUEUED
- [x] 1.4 Update the TS SDK command map (silent-bypass path removed; kv.set/kv.del now carry clientId — they dropped it even over HTTP), the Python SDK (client_id guard replaced by TXQUEUE wrapping), and the C# SDK mapper — all three wrap queueable writes in TXQUEUE and refuse the rest
- [x] 1.5 Add MULTI → queued write → EXEC/DISCARD parity S2S tests on all three transports in each SDK — Python parity suite (27 tests, rpc+resp3 roundtrip/discard/refusal), TS transactions.s2s.test.ts (7 tests ×3 transports), C# smoke harness E2E on all three transports + mapper unit tests

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — transports.md (TXQUEUE row + queued-writes note replacing the limitation), README (exception removed), CHANGELOG 1.3.0 (Added: TXQUEUE; Fixed bullet rewritten)
- [x] 2.2 Write tests covering the new behavior — server dispatch tests (RESP3 ×3 + RPC ×1 covering queue→exec, discard, no-MULTI, unqueueable) + SDK mapper unit tests (TS 3, C# 2, Python 3) + parity S2S suites
- [x] 2.3 Run tests and confirm they pass — full workspace + all SDK suites incl. S2S green (final validation before commit)
