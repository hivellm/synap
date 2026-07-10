## 1. Implementation
- [ ] 1.1 Blocking ops BLPOP/BRPOP/BZPOPMIN (client-wait + notify mechanism)
- [ ] 1.2 Pattern pub/sub PSUBSCRIBE + keyspace notifications
- [x] 1.3 Collection cursors HSCAN/SSCAN/ZSCAN
- [x] 1.4 LFU eviction policy (counter with decay)
- [ ] 1.5 Evaluate/implement IO threads (measure vs the sharded model first)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
