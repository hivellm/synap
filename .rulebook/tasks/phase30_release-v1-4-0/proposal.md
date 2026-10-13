# Proposal: phase30_release-v1-4-0

Source: docs/analysis/next-release-review-2026-10-12/ (release vehicle for every finding)
Target release: 1.4.0

## Why

The user decided that every finding of the next-release review ships in a
single version. phase15 through phase29 change authorization semantics on
every transport, the WAL and snapshot formats, and Hub mode startup rules.
Persisted data from 1.3.x stays readable, so this is a minor release (1.4.0)
rather than a major one. Downgrading after 1.4.0 has written data is not
supported, which operators must be told before upgrading.

## What Changes

- Every artifact moves to 1.4.0: Cargo workspace, TypeScript, Python, C# and
  PHP SDKs, and the GUI.
- A 1.4.0 CHANGELOG entry and a migration guide cover the authorization
  changes per transport, the WAL and snapshot format versions, the no-downgrade
  rule and backup step, the Hub listener policy, rate limiting now being
  enforced, the CLI `PING` exit status, and the read-only GUI configuration
  view.
- The release gate runs `cargo test-full` and the SDK E2E and S2S suites
  against the release build.
- An upgrade test starts 1.4.0 on a data directory written by 1.3.2.
- `release/v1.4.0` is prepared. Pushing and tagging happen only with the
  user's authorization.

## Impact

- Affected specs: release
- Affected code: version fields across the workspace, SDKs and GUI,
  `CHANGELOG.md`, `docs/`
- Dependencies: phase14 and phase15 through phase29
- Breaking change: YES. See the migration guide. 1.4.0 reads 1.3.x data, but
  1.3.x cannot read data written by 1.4.0.
- User benefit: every review fix arrives together, with one upgrade path to
  follow.
