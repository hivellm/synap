## 1. Dependency swap

- [x] 1.1 Add `hivellm-thunder` to `sdks/python/pyproject.toml` and move `msgpack` to the dev extra (still used by the transport tests as an independent codec, no longer by the transport)
- [x] 1.2 Add `synap_config()` exporting the Synap `Config` (scheme `synap`, port 15501, `AUTH_COMMAND`, push enabled, `RESP3_PREFIXES`, 512 MiB cap)
- [x] 1.3 Type-check clean — `mypy synap_sdk/transport_rpc.py` reports nothing; a `[[tool.mypy.overrides]]` entry suppresses the *import* error caused by `thunder_rpc` shipping no `py.typed` marker (filed upstream as hivellm/thunder#7, with a comment to remove the override once fixed)

## 2. Transport rewrite

- [x] 2.1 Rewrite `synap_sdk/transport_rpc.py` as an adapter over `thunder_rpc.AsyncClient`
- [x] 2.2 Rewrite `_to_wire`/`_from_wire` over Thunder's `Value` — decoding `Bytes` is Thunder's job now and covers both `bin` and the legacy int-array form; `bool` is still checked before `int` because in Python `bool` is an `int` subclass
- [x] 2.3 Route credentials through `ClientConfig`/`Credentials` instead of a hand-written AUTH frame
- [x] 2.4 Map Thunder's typed errors onto the SDK's exception types
- [x] 2.5 Delete the superseded framing, socket and reconnect code

## 3. Push path

- [x] 3.1 Consume SUBSCRIBE push frames through the client's push hook, registered *before* the command is sent, so a message published between the acknowledgement and the reader starting cannot be lost

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [x] 4.1 Update or create documentation covering the implementation — `sdks/python/README.md` (new "The `synap://` transport is Thunder" section) and `CHANGELOG.md` (Changed/Added/Security)
- [x] 4.2 Write tests covering the new behavior — `tests/test_transport.py` rewritten for Thunder's `Value` and the canonical array-shaped request encoding, plus a new test asserting an over-cap length prefix is refused. The fake servers still decode with `msgpack` directly, so they prove wire compatibility rather than self-consistency.
- [x] 4.3 Run tests and confirm they pass — 165 passed, 7 no-server cases not run, 60 S2S deselected. `ruff` reports the same 2 findings on the changed files as before the swap (both pre-existing), and the repository total went 142 → 139.
