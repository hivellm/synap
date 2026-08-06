# A published crate version can be a placeholder — check before scoping a major bump
**Source**: manual
**Date**: 2026-08-05
**Related Task**: phase3_dependency-audit-all-sdks
**Tags**: rust, cargo, bincode, dependency-audit, false-positive
`cargo outdated` reported `bincode 2.0.1 -> 3.0.0`, which looked like a serious migration: bincode owns the on-disk snapshot, stream WAL and inter-node quota framing, so a format change would have been a data-compatibility problem. It is not a release at all — bincode 3.0.0 is a squatted placeholder whose `src/lib.rs` is a single `compile_error!("https://xkcd.com/2347/")`, and it has no `serde` feature, so the first symptom is a confusing feature-resolution error rather than an honest "this version is empty". Before scoping work around a major bump reported by an outdated-checker, resolve it once and read the failure: `cargo add <crate>@<ver> --dry-run` surfaces missing features immediately, and a build failure inside the dependency's own `lib.rs:1` means the version is not real. Same class of trap as a yanked-but-listed version.
