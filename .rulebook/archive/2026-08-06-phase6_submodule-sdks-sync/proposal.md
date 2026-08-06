# Proposal: phase6_submodule-sdks-sync

## Why

The PHP and Go SDKs are git submodules pointing at `hivellm/synap-sdk-php` and
`hivellm/synap-sdk-go`, and both pointers are still on **v1.2.3** while the
server and the four in-tree SDKs are on 1.3.x. Two consequences surfaced during
the 1.3.1 dependency sweep (phase3): the Go SDK still pins
`github.com/hivellm/thunder-go v0.2.1` while every other end of the wire is on
Thunder 0.2.2, and neither submodule carries the 1.3.0 protocol work
(`TXQUEUE` transactional writes, the RESP3 stream family) or the 1.3.1
`stream.stats` wipe discriminator. Nothing in this repository can fix that:
the code lives in the other two repositories, and this repo only records a
commit id.

A clean `composer install` for the PHP SDK now resolves Guzzle 7.15.3, which is
free of the two advisories the pinned lock carried (`composer.lock` is
gitignored there, so no change was needed), but that verification was done
against the v1.2.3 tree — the same audit has to be re-run once the submodule
moves forward.

## What Changes

- In `synap-sdk-go`: bump `github.com/hivellm/thunder-go` to v0.2.2, run
  `go test ./...` and `govulncheck ./...`, and cut a 1.3.x release.
- In `synap-sdk-php`: verify the 1.3.0/1.3.1 protocol surface (TXQUEUE, RESP3
  streams, `stream.stats` `created_at`/`generation`), run `composer audit` on a
  fresh resolve, and cut a 1.3.x release.
- In this repository: advance both submodule pointers to the new tags and
  record the versions in the release notes.

## Impact

- Affected specs: none in this repository
- Affected code: `sdks/go` and `sdks/php` submodule pointers, `.gitmodules`
  stays unchanged
- Breaking change: NO
- User benefit: PHP and Go users get the same protocol surface and the same
  Thunder version as every other SDK, instead of silently running a
  three-release-old client.
