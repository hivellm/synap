# Submodule SDK pins drift ahead of their published tags
**Source**: manual
**Date**: 2026-07-19
**Related Task**: phase19_thunder-release-1-1-0
**Tags**: release, submodules, sdk, go, php
At 1.2.0 release time both submodule SDKs pointed at commits *ahead* of their own published tags: `sdks/go` was pinned at `74fe62b` while `v1.2.0` pointed at `6799dac`, and `sdks/php` at `13cbd21` while `v1.2.1` pointed at `e8a4dcb`. In both cases the two orphaned commits were `feat: kv watch` plus its docs — the headline feature of the release.

Publishing at the existing tags would have shipped a "1.2.0" Go and PHP SDK without KV watch, and different from what the interop matrix actually exercised (the matrix drives the working tree, not the tag).

Nothing catches this. The parent repo's CI only fails when a pinned submodule commit is *unreachable* on its remote ("upload-pack: not our ref"), never when it is reachable but untagged. `git submodule status` does show it — the `v1.2.0-2-g74fe62b` suffix literally counts the commits past the nearest tag — but only if you read it.

Before publishing any submodule SDK, run `git submodule status` and treat any `-N-g<sha>` suffix as "this needs a new tag first". Go compounds the cost: a Go module tag is immutable once the proxy has fetched it, so `v1.2.0` could not be re-pointed and a fresh `v1.2.1` had to be cut.</content>
