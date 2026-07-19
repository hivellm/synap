## 1. Implementation
- [x] 1.1 Provision a Redis 7 + redis-benchmark environment (WSL/Docker/Linux)
- [x] 1.2 Run redis-benchmark: Synap RESP3 vs Redis 7, -P 1 and -P 16 (GET/SET/INCR/LPUSH/LRANGE/SADD)
- [x] 1.3 Run native SynapRPC bench (cargo bench --bench protocol_bench)
- [x] 1.4 mimalloc A/B (kv_bench with vs without --features mimalloc)
- [x] 1.5 Replace stale 0.9.0/HTTP tables in docs/benchmarks/redis-vs-synap.md with fresh numbers

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
