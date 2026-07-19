## 1. Implementation
- [x] 1.1 Add AUTH handling + authenticated flag to the SynapRPC connection loop
- [x] 1.2 Reject unauthenticated commands on synap_rpc (per-command ACL refinement tracked in phase6h)
- [x] 1.3 Wire AppState auth into RESP3 check_auth; default authenticated to !require_auth; validate AUTH against real users
- [x] 1.4 Reject unauthenticated commands on RESP3 (per-command ACL refinement tracked in phase6h)
- [x] 1.5 Replace SHA-512 password hashing with bcrypt + per-user salt in auth/user.rs
- [x] 1.6 Use constant-time comparison for password verification
- [x] 1.7 Add verify-and-rehash migration for existing SHA-512 hashes on next successful login
- [x] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/security-auth.md)
- [x] 2.2 Write tests covering the new behavior (salted bcrypt hash + verify; legacy-hash format)
- [x] 2.3 Run tests and confirm they pass
