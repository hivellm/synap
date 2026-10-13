## 1. Implementation
- [x] 1.1 Add a coverage check that decides whether a set of permissions covers a requested resource pattern and action, including wildcard patterns
- [x] 1.2 Require an authenticated issuer in `auth_create_key` when authentication is enabled
- [x] 1.3 Reject requested grants the issuer's effective permissions do not cover; let admin users grant any permission
- [x] 1.4 Bound a key minted by an API key to the issuer key's permissions and expiry
- [x] 1.5 Reject unknown permission actions and unparsable `allowed_ips` entries with 400
- [x] 1.6 Make sure a rejected request stores no key

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
