# Rust `target/` hygiene

Cargo never garbage-collects `target/`. Stale object files, incremental caches,
and compiled rlibs from **old dependency versions** accumulate indefinitely until
something deletes them. Combined with the dev profile's default **full debuginfo**
for every workspace crate *and* every dependency, `target/` can grow without bound
— the sibling `hivellm/cortex` repo reached **500+ GB** (half a 1 TB SSD) before
anyone noticed.

This repo mitigates that with three levers: leaner debuginfo, a periodic sweep,
and disabling incremental compilation in CI.

## 1. Profile settings (`Cargo.toml`)

```toml
[profile.dev]
debug = "line-tables-only"   # keep file:line in panics/backtraces, drop the rest

[profile.release]
strip = true                 # drop the residual symbol table from release binaries
```

`debug = "line-tables-only"` is the single biggest size lever. It keeps `file:line`
information in panics and backtraces (so debugging is unaffected for the common
case) but omits full variable/type debuginfo. It also yields roughly **30–40%
faster incremental rebuilds** because there is far less debuginfo to write and
re-link. If you need to step through variables in a debugger, temporarily override
with `RUSTFLAGS="-C debuginfo=2"` or a local `[profile.dev]` tweak.

`strip = true` on release removes the residual symbol table from shipped binaries.
(This repo also sets `lto = "fat"`, `codegen-units = 1`, and `panic = "abort"` on
release for size/speed.)

## 2. Periodic sweep (`scripts/sweep-target.*`)

[`cargo-sweep`](https://github.com/holmgr/cargo-sweep) removes artifacts that
haven't been *accessed* in N days **without** breaking incrementality — the hot
set (recently touched artifacts) stays, so your next build is still fast.

```bash
# Linux/macOS
scripts/sweep-target.sh            # remove artifacts unused for 14 days (default)
scripts/sweep-target.sh 30         # custom retention
scripts/sweep-target.sh --dry-run  # preview, delete nothing
scripts/sweep-target.sh --clean    # full `cargo clean` (reclaim everything)
```

```powershell
# Windows
scripts/sweep-target.ps1
scripts/sweep-target.ps1 -Days 30
scripts/sweep-target.ps1 -DryRun
scripts/sweep-target.ps1 -Clean
```

Both scripts install `cargo-sweep` automatically on first use
(`cargo install cargo-sweep --locked`) and print the `target/` size before and
after.

## 3. Disable incremental compilation in CI

CI runners start from a cold cache, so incremental compilation only *adds*
artifacts (and the metadata to track them) and slows the build. The Rust
workflows set:

```yaml
env:
  CARGO_INCREMENTAL: 0
```

in `.github/workflows/rust-ci.yml`, `rust-lint.yml`, and `rust-test.yml`.

## 4. Scheduling the sweep (optional)

Keep `target/` bounded with zero manual effort by scheduling the sweep.

**Linux/macOS (cron)** — sweep daily at 03:00, retaining 14 days:

```cron
0 3 * * * cd /path/to/synap && scripts/sweep-target.sh 14 >/dev/null 2>&1
```

**Windows (Task Scheduler)** — daily at 03:00:

```powershell
$action  = New-ScheduledTaskAction -Execute 'pwsh.exe' `
  -Argument '-File C:\path\to\synap\scripts\sweep-target.ps1 -Days 14'
$trigger = New-ScheduledTaskTrigger -Daily -At 3am
Register-ScheduledTask -TaskName 'synap-sweep-target' -Action $action -Trigger $trigger
```

## References

- Disable/limit debuginfo to shrink `target/` and speed compiles (Rust perf-team,
  Kobzol 2025):
  <https://kobzol.github.io/rust/rustc/2025/05/20/disable-debuginfo-to-improve-rust-compile-times.html>
- `cargo-sweep`: <https://github.com/holmgr/cargo-sweep>
- Cargo profiles reference:
  <https://doc.rust-lang.org/cargo/reference/profiles.html>
