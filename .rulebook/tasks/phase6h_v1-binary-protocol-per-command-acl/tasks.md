## 1. Implementation
- [x] 1.1 auth::command_requires_admin classifier covering the admin/destructive commands across RESP3 + SynapRPC (shared table)
- [x] 1.2 Both binary protocols track the resolved User post-AUTH and check the ACL before dispatch
- [x] 1.3 Destructive/admin commands (FLUSHALL/FLUSHDB/SWAPDB, CONFIG/SHUTDOWN/DEBUG/RESET, SAVE/BGSAVE, SLAVEOF/REPLICAOF/FAILOVER, ACL/MODULE, SCRIPT.FLUSH/KILL, CLUSTER) gated behind admin → NOPERM for non-admins
- [x] 1.4 Gate: cargo check, clippy -D warnings, fmt --check (green)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/security-auth.md per-command ACL section + CHANGELOG)
- [x] 2.2 Write tests covering the new behavior (command_requires_admin classifier: destructive require admin; ordinary do not — the security-critical decision the gate enforces)
- [x] 2.3 Run tests and confirm they pass (full workspace suite: 1716 passed, 0 failed)
