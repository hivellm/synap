# Proposal: phase27_rest-kv-string-encoding-fixes

Found by: manual pre-publication test of the 1.2.0 release image (2026-07-19).
Both defects date to `75f4cff` (Release/v1.0.0) and are shipped today.

## Why

The REST KV handlers disagree with the store about how a string value is
encoded, and the disagreement is silent.

`kv_set` stores a JSON string as **raw UTF-8** (`s.as_bytes()`), reserving
JSON encoding for non-string values. Two other handlers never got that rule:

1. **`SET` with `get: true` never returns `old_value` for a string.** The
   response conversion is
   `old_value.and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())`.
   For a stored string the bytes are `first`, which is not valid JSON, so the
   parse fails, `.ok()` discards the error, and `skip_serializing_if` omits the
   field. The caller cannot distinguish "there was no previous value" from
   "the previous value could not be decoded". Verified against the release
   image: setting `t:a` to `second` with `get: true` over a stored `first`
   answered `{"success":true,"key":"t:a","written":true}` — no `old_value`.
   It only appears to work when the stored value happens to be valid JSON
   (numbers, objects), which is why it survived review.

2. **`APPEND` corrupts the value.** `kv_append` runs
   `serde_json::to_vec(&req.value)`, so appending the string `cd` writes
   `"cd"` — with the quotes. Verified: `SET ab` then `APPEND cd` yields
   `ab"cd"` and reports length 6. The stored value is now neither what the
   caller appended nor valid JSON.

Both are data-correctness bugs on the primary public API, not cosmetic.

## What Changes

- `kv_set`: decode `old_value` with the inverse of the write rule — return the
  bytes as a JSON string when they are valid UTF-8 and were not stored as
  JSON, instead of silently dropping them.
- `kv_append`: encode the value with the same rule `kv_set` uses — raw UTF-8
  for strings, JSON for everything else.
- Regression tests pinning both, including the round trip that currently
  corrupts (`SET` + `APPEND` + `GET`) and the `get: true` path over a plain
  string.

## Impact

- Affected specs: `specs/rest-kv-string-encoding/spec.md` (ADDED)
- Affected code: `crates/synap-server/src/server/handlers/kv.rs`
- Breaking change: NO for correct clients. A client that today strips the
  quotes `APPEND` injects, or that relies on `old_value` being absent, would
  see the corrected values — which is the point of the fix.
- User benefit: `APPEND` stops corrupting data, and `SET ... GET` returns the
  previous value it has always advertised.
