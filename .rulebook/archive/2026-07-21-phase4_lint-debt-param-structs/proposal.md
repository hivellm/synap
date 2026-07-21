# Proposal: phase4_lint-debt-param-structs

## Why

Clippy passes only because 71 `#[allow(...)]` suppressions silence it. Setting
aside the `dead_code` group (owned by phase3_cluster-scaffolding-triage), the
remaining structural suppressions point at genuine design friction rather than
false positives:

- `12× clippy::too_many_arguments` — functions taking 8+ positional parameters.
  These are error-prone at every call site (argument-order bugs the compiler
  cannot catch) and are exactly what a parameter struct fixes.
- `4× clippy::type_complexity` — deeply nested generic types that a `type` alias
  makes readable.
- `2× clippy::too_many_lines`, `6× clippy::while_let_loop`,
  `2× clippy::result_unit_err` — smaller cleanups.

Each suppression is a lint the project's own rules (`AGENTS.md`: "Warnings are
errors") chose to mute rather than fix. This task removes the muting by fixing
the underlying cause, not by relaxing the lint.

## What Changes

- For each `too_many_arguments` site, introduce a parameter struct (e.g.
  `struct FooParams { ... }`) or builder, passed by reference, and remove the
  `#[allow]`. Call sites become named-field construction — self-documenting and
  order-safe.
- For each `type_complexity` site, introduce a `type` alias in the module and
  remove the `#[allow]`.
- Address the `result_unit_err` sites by returning a real error type, and the
  `while_let_loop`/`too_many_lines` sites by the idiomatic rewrite clippy
  suggests.
- `dead_code` suppressions are **out of scope** here — they are handled by
  phase3 (cluster) and any residual ones are re-evaluated after phase3 lands.

Work proceeds one suppression-family at a time, `clippy -D warnings` + tests
green after each, so a regression is isolated.

## Impact
- Affected specs: none (no behavior change)
- Affected code: the ~20 call sites across `crates/synap-core/src` and
  `crates/synap-server/src` that currently carry the targeted `#[allow]`
- Breaking change: NO for public wire/API; internal function signatures change
  (parameter structs) but these are crate-internal
- User benefit: fewer silenced lints, order-safe call sites, and signatures that
  document themselves — reducing a whole class of argument-position bugs
