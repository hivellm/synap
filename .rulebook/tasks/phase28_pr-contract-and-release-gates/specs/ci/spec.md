# Pull Request and Release Gates

## ADDED Requirements

### Requirement: Contract checks before merge
Pull request CI MUST run the SDK tests of every language, GUI checks, SDK
contract tests against a built server, and the interop harness, and MUST fail
when any of them fails.

#### Scenario: Wrong command name
Given a pull request that renames `key.type` on the server without updating the GUI
When CI runs
Then a contract check fails

#### Scenario: Changed amount mapping
Given a pull request that maps `kv.incr` with an amount back to plain `INCR`
When CI runs
Then a contract check fails

### Requirement: Enforced coverage
CI MUST measure coverage per component and MUST fail when a component drops
below its recorded floor.

#### Scenario: Coverage drop
Given a pull request that removes tests and lowers a component below its floor
When CI runs
Then the coverage gate fails naming the component

### Requirement: Reproducible builds
Release and CI builds MUST use the committed `Cargo.lock` with `--locked` and
the toolchain pinned in `rust-toolchain.toml`.

#### Scenario: Fresh checkout
Given a fresh checkout of a release tag
When the release build runs
Then it resolves exactly the dependency versions in the committed `Cargo.lock`
And it uses the pinned toolchain
