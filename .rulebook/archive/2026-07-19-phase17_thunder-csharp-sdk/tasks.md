## 1. Outcome

This task is **blocked on upstream packaging** and is superseded by
`phase21_thunder-csharp-sdk-swap`. No Synap code changed.

- [x] 1.1 Establish whether `HiveLLM.Thunder` can be consumed — it cannot: NuGet carries only 0.1.0 while crates.io, npm and PyPI all carry 0.2.0, and the Thunder repo itself declares 0.2.0. Pinning 0.1.0 would leave the C# SDK a version behind Synap's other three and missing the zero-length-frame / keep-alive handling from thunder#6, which is a live interop difference.
- [x] 1.2 File the blocker upstream — hivellm/thunder#8, with the registry comparison and a suggested CI guard that fails when a published version lags the repo
- [x] 1.3 Correct this task's premise — the proposal asserted the SDK used `MessagePack.Typeless` on untrusted wire data, generalizing from Thunder's family-wide C# analysis. `grep -rn Typeless sdks/csharp/` returns nothing; Synap's C# SDK hand-rolls its own MessagePack helper and has no MessagePack package reference at all. The real defect is the unbounded allocation from the length prefix, recorded in the follow-up task.
- [x] 1.4 Create the follow-up task carrying the swap — `phase21_thunder-csharp-sdk-swap`, blocked on hivellm/thunder#8

## 2. Tail (docs + tests — check or waive with tailWaiver)

- [x] 2.1 Update or create documentation covering the implementation — `CHANGELOG.md` gains a "Not in this release" section stating that the C# SDK keeps its hand-written transport, why, and the upstream issue
- [x] 2.2 Write tests covering the new behavior — none apply: no Synap code changed. The C# SDK's existing suite is unaffected and the behavior it covers is unchanged.
- [x] 2.3 Run tests and confirm they pass — the C# suite was not re-run because nothing in `sdks/csharp/` was touched; the swap's tests belong to the follow-up task
