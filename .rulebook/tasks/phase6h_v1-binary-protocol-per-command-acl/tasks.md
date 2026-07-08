## 1. Implementation
- [ ] 1.1 Define a command -> (resource, action) permission map for the RESP3 and SynapRPC command sets
- [ ] 1.2 Check the authenticated user's ACL before executing each command on both binary protocols
- [ ] 1.3 Gate destructive/admin commands (FLUSHALL, FLUSHDB, CONFIG, cluster ops) behind admin
- [ ] 1.4 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (non-admin denied unpermitted command; admin allowed)
- [ ] 2.3 Run tests and confirm they pass
