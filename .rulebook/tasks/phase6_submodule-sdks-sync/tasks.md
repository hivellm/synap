## 1. Go SDK (hivellm/synap-sdk-go)

- [ ] 1.1 Bump `github.com/hivellm/thunder-go` to v0.2.2
- [ ] 1.2 Run `go test ./...` and `govulncheck ./...`
- [ ] 1.3 Cover the 1.3.x protocol surface (TXQUEUE, RESP3 streams, `stream.stats` generation)
- [ ] 1.4 Cut a 1.3.x release tag

## 2. PHP SDK (hivellm/synap-sdk-php)

- [ ] 2.1 Verify the 1.3.x protocol surface (TXQUEUE, RESP3 streams, `stream.stats` generation)
- [ ] 2.2 Run `composer audit` on a fresh resolve and the PHPUnit suite against a live server
- [ ] 2.3 Cut a 1.3.x release tag

## 3. This repository

- [ ] 3.1 Advance the `sdks/go` and `sdks/php` submodule pointers to the new tags
- [ ] 3.2 Record both SDK versions in the release notes

## 4. Tail (docs + tests)

- [ ] 4.1 Update or create documentation covering the implementation
- [ ] 4.2 Write tests covering the new behavior
- [ ] 4.3 Run tests and confirm they pass
