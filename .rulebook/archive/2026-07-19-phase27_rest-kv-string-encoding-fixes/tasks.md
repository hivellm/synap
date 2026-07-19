## 1. Implementation
- [x] 1.1 `kv_append`: encode the value with the same rule `kv_set` uses — raw UTF-8 for strings, JSON for everything else — so `SET ab` + `APPEND cd` yields `abcd` and not `ab"cd"`
- [x] 1.2 `kv_set`: return `old_value` for a value stored as raw UTF-8 instead of dropping it when `serde_json::from_slice` fails, keeping the JSON branch for values that were stored JSON-encoded
- [x] 1.3 Gate: `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` — all clean
- [x] 1.4 Scope grew during implementation: `kv_setrange` and `kv_getset` carried the identical defect (`serde_json::to_vec(&req.value)`), so `GETSET x` stored `"x"` with quotes. Fixing only append/set would have left two of the four writers disagreeing with the store. All four now share `encode_value_bytes`, with `decode_stored_value` as its documented inverse

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — `docs/users/kv-store/ADVANCED.md` gained a "How values are encoded" section stating the raw-UTF-8-for-strings rule and the `old_value` contract; the same section's curl examples had the path segments reversed (`/kv/append/user:1` instead of `/kv/{key}/append`) and are corrected. CHANGELOG entry under 1.2.0
- [x] 2.2 Write tests covering the new behavior — 6 strict regression tests in `string_integration_tests.rs` asserting equality rather than containment: append/getset/setrange round trips, and `get: true` over a plain string, over a JSON value, and over an absent key
- [x] 2.3 Run tests and confirm they pass — 16/16 in `string_integration_tests`, full synap-server suite green (62 binaries), synap-core 433 green. Verified end to end against a rebuilt release image: `APPEND cd` on `ab` now returns `abcd` (length 4) and `SET ... get:true` returns `"old_value":"first"`

## 3. Note on how this shipped
- [x] 3.1 The pre-existing `test_string_append_creates_and_appends` asserted `stored_value.contains("hello")`, which passes just as well against the corrupted `"hello"`. The `GETSET` test went further and pinned the corrupted form outright (`assert_eq!(value_new, "\"initial\"")`) — while asserting the un-quoted form two lines earlier for a `SET`-written value. The test disagreed with itself and was believed anyway; both assertions are corrected
