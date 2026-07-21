## 1. Implementation
- [ ] 1.1 Inventory the non-dead_code `#[allow]` sites (`too_many_arguments`, `type_complexity`, `too_many_lines`, `while_let_loop`, `result_unit_err`) with file:line
- [ ] 1.2 Replace each `too_many_arguments` (12) with a parameter struct passed by reference; remove the `#[allow]`
- [ ] 1.3 Replace each `type_complexity` (4) with a `type` alias; remove the `#[allow]`
- [ ] 1.4 Fix `result_unit_err` (2) with a real error type and `while_let_loop`/`too_many_lines` with the idiomatic rewrite; remove those `#[allow]`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (note any internal signature changes in the CHANGELOG "Changed" if user-visible)
- [ ] 2.2 Write tests covering the new behavior (existing suite is the regression guard; add a test only where a `result_unit_err` fix introduces a new error path)
- [ ] 2.3 Run tests and confirm they pass (after each suppression-family: `cargo check` + `clippy -D warnings` + full suite, green)
