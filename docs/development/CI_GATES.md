# CI gates: pull requests, coverage and releases

What has to pass before a change merges, what a release checks before it
publishes, and how builds stay reproducible. The wiring is guarded by
`scripts/ci/test_ci_gates.py`, which runs on every pull request.

## Reproducible builds: lockfile and toolchain

- **`Cargo.lock` is committed.** Synap ships binaries, so the lockfile is part
  of the source. Every cargo command in CI, the release workflow, SDK
  publishing and the Dockerfile passes `--locked`, so a build fails instead of
  quietly resolving a different dependency graph. To change dependencies, run
  `cargo update` (or `cargo update -p <crate>`), build and test, and commit
  `Cargo.lock` with the change.
- **The toolchain is pinned by date** in `rust-toolchain.toml`
  (`nightly-2026-07-23`, which is rustc `1.99.0-nightly (6f72b5dd5 2026-07-22)`;
  a nightly's dist date is one day after the commit date `rustc -vV` prints).
  No workflow names a toolchain. They all install it through
  `.github/actions/setup-rust`, which reads the channel and components from
  that file, and the Dockerfile reads the same file. To move to a newer
  nightly, change the date there, run the full gate locally, and commit.
- Tags cut before the lockfile was committed (1.3.2 and earlier) cannot be
  rebuilt with `--locked`. The workflows apply from the first tag that
  contains them.

## Pull-request gates

Workflows run on pushes and pull requests to `main` and `develop`.

| Workflow / job | What it runs |
|---|---|
| `rust-ci.yml` test | `cargo check`, unit tests, `cargo test-full --tests` (integration), doc tests, on Linux, Windows and macOS |
| `rust-ci.yml` coverage | `cargo llvm-cov --lib --workspace --locked`, then the coverage gate for synap-core, synap-server and synap-sdk |
| `rust-lint.yml` | `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`, `cargo deny --locked` |
| `sdk-gui-contract.yml` server | one release build of `synap-server`, shared with the jobs below |
| `sdk-gui-contract.yml` sdk-typescript | `npm run type-check`, `npm run test:unit` with coverage, coverage gate, `npm run build` |
| `sdk-gui-contract.yml` sdk-python | `pytest` with coverage, coverage gate |
| `sdk-gui-contract.yml` sdk-csharp | `dotnet test` with coverage, coverage gate |
| `sdk-gui-contract.yml` sdk-php | the `sdks/php` submodule's PHPUnit suite against the server (the suite mixes unit and server-to-server tests, and that repository has no CI of its own) |
| `sdk-gui-contract.yml` gui | `npm run type-check`, `npm run lint`, `npm run build`, `npm test` (API client contract tests and inspector smoke tests against the server) |
| `sdk-gui-contract.yml` contract | TypeScript E2E suite over HTTP, SynapRPC and RESP3; TypeScript and Python server-to-server suites; `scripts/interop/run-matrix.py --strict` over every cell (Rust, TypeScript, Python, C#, Go, PHP, pre-Thunder replay) |
| `sdk-gui-contract.yml` gate-scripts | tests of the coverage gate, the CI wiring checks, the release-workflow checks, the tests of the performance baseline script (`scripts/bench/test_perf_baseline.py`) |

The Go SDK lives in its own repository (`sdks/go` submodule). Its suite runs
there; here it is exercised through its interop cell.

### What catches a broken contract

- **A server command renamed while a client still sends it.** The GUI tests
  run the real API client against the server built from the pull request.
  Renaming `key.type` in the dispatcher fails 23 of them with
  `Unknown command: key.type`.
- **A native command map that drops a counter amount.** The TypeScript unit
  tests pin the mapping (`kv.incr` with an amount → `INCRBY`), and the E2E
  suite asserts `incr(counter, 10)` and `decr(counter, 4)` against the server
  on every transport. Mapping `kv.incr` back to a one-argument `INCR` fails
  three unit tests and the SynapRPC and RESP3 E2E cases (`expected 4 to be
  13`). The Rust, Python and C# maps have their own unit tests, and the server
  has `counter_amount_parity_tests`.

### GUI lint baseline

The GUI lints with ESLint 9's flat config (`gui/eslint.config.mjs`: Vue 3
essential, `eslint:recommended`, typescript-eslint recommended). The 85
findings that existed when the gate was introduced (69 `no-explicit-any`, 14
`no-unused-vars`, 2 `no-case-declarations`) are recorded in
`gui/eslint-suppressions.json`, ESLint's bulk suppressions. A new finding
fails `npm run lint`, and so does a recorded one that no longer occurs. After
fixing findings, run `npm run lint:prune` to shrink the file. Never grow it to
get a change through.

Type-checking runs `tsc --noEmit` over the TypeScript sources. `.vue`
single-file components are parsed by the linter and compiled by the build,
but `tsc` does not type-check their templates or script blocks; that would
need `vue-tsc`.

## Coverage

`scripts/ci/coverage_gate.py` reads `.github/coverage-floors.json` and fails
when a component's line coverage drops below its floor, naming the component.
It also prints how far each component is from the 95% policy.

Each floor is the coverage measured on 2026-10-13 in the mode CI runs,
truncated to one decimal. No component meets the 95% policy yet; the floors
stop further drops while the gap closes.

| Component | Mode | Measured | Floor | Gap to 95% |
|---|---|---|---|---|
| synap-core | `cargo llvm-cov --lib` (unit tests), Linux | 11747/13770 lines, 85.31% | 85.3 | 9.7 points |
| synap-server | `cargo llvm-cov --lib` (unit tests), Linux | 15325/32098 lines, 47.74% | 47.7 | 47.3 points |
| synap-sdk (Rust) | `cargo llvm-cov --lib` (unit tests), Linux | 2141/4752 lines, 45.05% | 45.0 | 50.0 points |
| TypeScript SDK | `npm run test:unit -- --coverage` | 864/1263 lines, 68.41% | 68.4 | 26.6 points |
| Python SDK | `pytest` (S2S suites skip without a server) | 1090/1752 lines, 62.21% | 62.2 | 32.8 points |
| C# SDK | `dotnet test --collect "XPlat Code Coverage"` (S2S tests skip) | 702/1833 lines, 38.30% | 38.2 | 56.7 points |

Not measured: the integration suites (`cargo test-full`) under coverage, the
GUI (its tests are contract tests and it has no coverage tooling), the PHP and
Go SDKs (separate repositories), synap-cli (no unit tests under `--lib`) and
synap-migrate (its unit tests run under `--lib`, but it has no floor yet). Two Linux runs of the Rust measurement differed by one line of
synap-server, which the one-decimal truncation absorbs.

When coverage rises, raise the floor in the same pull request; the gate
prints a notice when a component is a point or more above its floor. Never
lower a floor to make a pull request pass.

Reproduce locally:

```bash
cargo llvm-cov --lib --workspace --locked --lcov --output-path lcov.info
python scripts/ci/coverage_gate.py synap-core synap-server synap-sdk
(cd sdks/typescript && npm run test:unit -- --coverage --coverage.reporter=lcov)
python scripts/ci/coverage_gate.py sdk-typescript
```

The Rust floors were measured on Linux; on Windows and macOS different
`cfg` branches compile, so local numbers there differ slightly.

## Performance baseline

`perf-baseline.yml` runs `scripts/bench/perf_baseline.py` (see
[scripts/bench/README.md](../../scripts/bench/README.md)) on a schedule
(Mondays 04:17 UTC) and on manual dispatch. It never runs on pull requests
and has **no latency or throughput gate**: a job fails only when the
measurement itself breaks (a server that does not start or exits, a load
generator failure, error replies). Numbers are compared by a person, between
reports from the same runner.

Which job runs depends on the repository variable `PERF_BENCH_RUNNER`
(Settings > Secrets and variables > Actions > Variables):

| `PERF_BENCH_RUNNER` | Job | Runs on | What it runs | Artifact |
|---|---|---|---|---|
| set, e.g. `synap-perf` | `baseline` | that runner label | the full matrix, 5 runs (dispatch inputs `runs`, `duration`), `--runner-class controlled` | `perf-baseline-<run id>`, kept 90 days |
| unset | `smoke` | `ubuntu-latest` | one 1-second run of the matrix, `--runner-class shared` | `perf-smoke-not-comparable-<run id>`, kept 14 days |

The controlled runner is a machine reserved for this job: register a
self-hosted runner with a label of its own (for example `synap-perf`) on
hardware that runs nothing else while the job is active, then set
`PERF_BENCH_RUNNER` to that label. Any OS works as long as `bash` is on the
`PATH` (Git Bash on Windows): the job runs its steps in bash, as
`.github/actions/setup-rust` already requires. Its reports carry `runner.comparable: true`.
The smoke job only proves that the script, the load generator and the server
still work together on a shared runner, where neighbouring jobs make timing
meaningless; its report says `comparable: false` and its artifact name says so
too.

Both jobs install the pinned toolchain through `.github/actions/setup-rust`
and build with `cargo build --release --locked --package synap-server --bin
synap-server --example bench_loadgen`, then run the script with `--no-build`.

## Before publishing

`sdk-publish.yml` builds `synap-server` from the release tag once. Every
publish job then packs its artifact (`cargo package`, `npm pack`, the wheel,
the `.nupkg`) and runs `scripts/release/artifact-smoke/run.py`, which installs
the artifact into a scratch project outside the repository and runs a smoke
client against that server over HTTP, SynapRPC and RESP3: set/get, INCR/DECR
with and without an amount, delete, and an HTTP write read back over both
native transports. A failure stops that job before it publishes.

```bash
cargo build --release --locked -p synap-server
cargo package -p synap-sdk --locked
python scripts/release/artifact-smoke/run.py rust --artifact target/package/synap-sdk-<version>.crate
(cd sdks/typescript && npm run build && npm pack)
python scripts/release/artifact-smoke/run.py typescript --artifact sdks/typescript/hivehub-synap-<version>.tgz
```

The release binaries (`release.yml`) build with `locked: true` and the pinned
toolchain; the same source is exercised on every pull request by the contract
job's release build.
