## 1. Implementation
- [x] 1.1 Native health probe: synap-server --health-check (plain-std HTTP GET on /health, SYNAP_HEALTH_ADDR override, exit 0/1)
- [x] 1.2 Dockerfile runtime -> FROM scratch (rootfs prep stage: passwd/group, CA bundle, /data tree; non-root UID 1000; HEALTHCHECK via the binary)
- [x] 1.3 Rebuilt, smoke tested (healthy status via native probe, /health 1.0.0, RESP3 SET/GET/INCR), Scout 0C/0H/0M/0L (was 38 CVEs / 114 packages), 129MB -> 19.4MB; re-pushed 1.0.0 + latest (digest 8f733f4e)
- [x] 1.4 SDK CHANGELOGs: [1.0.0] entry added to rust/typescript/python/php/csharp (documenting the audit fixes + dep bumps); Go and Java changelogs created
- [x] 1.5 SDK READMEs: stale versions fixed (rust 0.1 -> 1.0, TS 0.2.0-beta.1 footer -> 1.0.0, Java 0.11.1 -> 1.0.0); quick-start examples in TS/Rust/PHP/C# now lead with the synap:// default transport (Python/Go/Java already did)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
