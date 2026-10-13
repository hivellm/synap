## 1. Implementation
- [ ] 1.1 Add a coverage check that decides whether a set of permissions covers a requested resource pattern and action, including wildcard patterns
- [ ] 1.2 Require an authenticated issuer in `auth_create_key` when authentication is enabled
- [ ] 1.3 Reject requested grants the issuer's effective permissions do not cover; let admin users grant any permission
- [ ] 1.4 Bound a key minted by an API key to the issuer key's permissions and expiry
- [ ] 1.5 Reject unknown permission actions and unparsable `allowed_ips` entries with 400
- [ ] 1.6 Make sure a rejected request stores no key

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
