# URL-scheme transport selection replaces multi-field builder options
**Source**: manual
**Date**: 2026-04-09
**Related Task**: phase3_full-rpc-resp3-parity-and-url-schemes
**Tags**: transport, url-scheme, deprecation, clippy, php, python, multi-sdk
When adding multi-transport support to SDKs (Rust, TS, Python, PHP, C#), the cleanest migration path is to encode the transport choice in the URL scheme rather than separate builder fields. Key lessons:

1. **Pattern**: Parse the scheme prefix in the constructor (`synap://` → SynapRPC, `resp3://` → RESP3, `http(s)://` → HTTP). No other fields required for the happy path.

2. **Deprecation**: Keep old builder methods but mark them deprecated (`#[deprecated]`, `@deprecated`, `[Obsolete]`, `trigger_error(E_USER_DEPRECATED)`) and emit warnings. They can call through to the new internals.

3. **Clippy -D warnings catches deprecated usage in own tests**: After adding `#[deprecated]` to builder methods, running clippy with `-D warnings` surfaces usages in test files. Fix by either removing the call (when `http://` already implies HTTP transport) or adding `#[allow(deprecated)]` to tests specifically validating the deprecated path.

4. **`len() >= 1` → `!is_empty()`**: clippy::len_zero fires on `fields.len() >= 1` patterns in test assertions. Always use `!is_empty()`.

5. **PHP S2S tests need explicit skip guard**: Tests annotated `@group s2s` still run unless excluded with `--exclude-group s2s`. Add a `markTestSkipped` in `setUp()` checking `SYNAP_S2S=true` to make them safe to run without a server.

6. **Python `SynapConfig` positional arg**: The config takes `base_url` as first positional arg; S2S tests written with `url=...` keyword break. Always use positional or match the actual param name.