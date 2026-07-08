## 1. Implementation
- [ ] 1.1 Add HELLO/AUTH handshake + authenticated flag to the SynapRPC connection loop
- [ ] 1.2 Enforce per-command ACL/permission check in synap_rpc dispatch before executing store ops
- [ ] 1.3 Wire AppState auth into RESP3 check_auth; default authenticated to !auth.required; validate AUTH against real users
- [ ] 1.4 Enforce ACL on RESP3 command dispatch for authenticated users
- [ ] 1.5 Replace SHA-512 password hashing with bcrypt + per-user salt in auth/user.rs
- [ ] 1.6 Use constant-time comparison for password verification
- [ ] 1.7 Add verify-and-rehash migration for existing SHA-512 hashes on next successful login
- [ ] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (security/auth docs + config)
- [ ] 2.2 Write tests covering the new behavior (unauth command rejected on both protocols; bcrypt verify; legacy-hash migration)
- [ ] 2.3 Run tests and confirm they pass
