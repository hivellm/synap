# Proposal: phase14_sdk-1.0-rpc-audit

## Why

The 1.0.0 server just shipped to Docker Hub. The 7 client SDKs (rust,
typescript, python, php, csharp, go, java) must be verified for the release:
every SDK at version 1.0.0, every SDK defaulting to the native SynapRPC
transport (not HTTP), test suites passing against the real 1.0.0 server image,
and dependencies reviewed/updated to current versions where warranted.

## What Changes

1. Version audit: every SDK manifest at 1.0.0.
2. Transport audit: SynapRPC (`synap://`, port 15501) is the default in every
   SDK; RESP3/HTTP remain available as opt-in.
3. Test run: each SDK's suite executed (unit + live against the
   `hivehub/synap:1.0.0` container) with available toolchains; failures fixed.
4. Dependency review: update each SDK's dependencies (npm / pip / composer /
   NuGet / go modules / maven) and the Rust workspace to current versions where
   compatible; note any deliberately held back.

## Impact

- Affected specs: none
- Affected code: sdks/* manifests and any fixes surfaced by the audit
- Breaking change: NO
- User benefit: trustworthy 1.0.0 SDKs — correct version, fastest transport by
  default, green tests, current dependencies
