## 1. Implementation
- [x] 1.1 Add a blocking-waiter registry to synap-core: one waiter registered on every key it blocks on, FIFO wake per key, hand-off of unused wake-ups when a waiter leaves, no lock taken by pushes while nobody waits
- [x] 1.2 Rewrite the list store's blocking pops (BLPOP, BRPOP, BRPOPLPUSH, plain and guarded) on the registry: register before the last emptiness check, wait on all keys at once, keep the guard only around pop attempts, keep timeout semantics
- [x] 1.3 Rewrite the sorted-set store's BZPOPMIN/BZPOPMAX (plain and guarded) on the registry and wake waiters on every write that adds members (ZADD, ZINCRBY, ZINTERSTORE, ZUNIONSTORE, ZDIFFSTORE)
- [x] 1.4 Unit tests in synap-core: wake on the second key, argument order when several keys hold data, timeout, many concurrent waiters each get one element, FIFO order on one key, no lost wake-up under a push/pop race, hand-off when a woken waiter leaves
- [ ] 1.5 Cross-transport integration test (RESP3 and SynapRPC, with and without the persistence layer) with a `[[test]]` entry gated on `full-tests`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
