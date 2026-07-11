## 1. Implementation
- [x] 1.1 Version audit: rust (workspace 1.0.0), typescript 1.0.0, python 1.0.0, csharp 1.0.0, java 1.0.0, php (git-tag versioned), go (module, git-tag) — all aligned
- [x] 1.2 Transport audit: SynapRPC (synap://, 15501) is the default in all 7 SDKs (TS, Python-documented, PHP, C#, Go, Java constructors verified); RESP3/HTTP opt-in via URL scheme
- [x] 1.3 Test runs vs the live 1.0.0 container: TS 467 pass (fixed kv.set object -> "[object Object]" bug, kv.delete API drift in tests, auth-required probe); Python 177 pass (fixed CRITICAL: KV module called the dead legacy /api/stream endpoint — every call silently returned None; rewritten on send_command with dual-shape normalization; transport mock tests aligned to the real map-framed wire format); Go unit+integration (all 3 transports live) pass; C# 96 pass; Rust 109 lib + 30 integration + 8 server s2s live pass. PHP/Java: static audit (no toolchain on this host) — transports, modules and versions verified
- [x] 1.4 Dependency review: TS (lockfile ERESOLVE fixed, typescript-eslint 8.63 aligned, minors updated, stale @types/uuid removed; TypeScript 6->7 deliberately held — new native compiler major); C# NetAnalyzers 8->9 (Json 10.x held for the net8.0 LTS target); Go testify 1.6->1.11 + tidy; Java jackson 2.17.1->2.18.2; Python floors already current; Rust workspace cargo update (105 crates), full test suite + clippy green after

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
