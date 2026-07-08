## ADDED Requirements

### Requirement: Crates directory layout
The workspace MUST place all first-party crates under `crates/`
(`crates/synap-server`, `crates/synap-cli`, `crates/synap-migrate`), with the root
manifest declaring `members = ["crates/*", "sdks/rust"]`, matching the Vectorizer/Nexus
convention.

#### Scenario: Workspace compiles after the move
Given the crates have been moved with `git mv` and no source edits
When `cargo check --workspace` and `cargo test` run
Then both complete with zero errors and the same test count as before the move

### Requirement: Zero behavior change in the skeleton phase
The skeleton move MUST NOT modify any `src/**` file content; only paths, manifests,
CI/Docker/helm/script references may change.

#### Scenario: Diff contains no source edits
Given the phase's final commit range
When `git diff --stat` is filtered to `src/**` content changes (excluding renames)
Then no content modifications appear
