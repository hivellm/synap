## 1. Go SDK (hivellm/synap-sdk-go)

- [x] 1.1 Bump `github.com/hivellm/thunder-go` to v0.2.2
- [x] 1.2 Run `go test ./...` and `govulncheck ./...`
- [x] 1.3 Cover the 1.3.x stream surface (SREAD mapping, SGETORCREATE, `stream.stats` generation)
- [x] 1.4 Implement transactional writes (MULTI/EXEC/TXQUEUE) — delivered in phase7
- [x] 1.5 Cut 1.3.1: CHANGELOG and an annotated v1.3.1 tag

## 2. PHP SDK (hivellm/synap-sdk-php)

- [x] 2.1 Correct the native pub/sub command names and map `stream.stats` to SSTATS
- [x] 2.2 Queue transactional writes through TXQUEUE (ADR 005), exposed as `$clientId` on every queueable write method
- [x] 2.3 Run `composer audit` on a fresh resolve and the PHPUnit unit suite
- [x] 2.4 Cut 1.3.1: CHANGELOG and an annotated v1.3.1 tag

## 3. This repository

- [x] 3.1 Advance the `sdks/go` and `sdks/php` submodule pointers to the new tags
- [x] 3.2 Record both SDK versions in the release notes

## 4. Tail (docs + tests)

- [x] 4.1 Update or create documentation covering the implementation
- [x] 4.2 Write tests covering the new behavior
- [x] 4.3 Run tests and confirm they pass
