## 1. Dependency swap

- [ ] 1.1 Add `hivellm-thunder` to `sdks/python/pyproject.toml` and remove the transport's direct `msgpack` dependency
- [ ] 1.2 Add a module exporting the Synap `Config` (scheme `synap`, port 15501, `AuthCommand`, push enabled)
- [ ] 1.3 Type-check (`mypy`/`pyright` as configured) clean

## 2. Transport rewrite

- [ ] 2.1 Rewrite `synap_sdk/transport_rpc.py` as an adapter over `thunder_rpc.Client`
- [ ] 2.2 Centralize SDK-value ↔ `thunder_rpc.Value` conversion, decoding `Bytes` from both `bin` and the legacy int-array form
- [ ] 2.3 Route credentials through `ClientConfig`/`Credentials` instead of a hand-written AUTH frame
- [ ] 2.4 Map Thunder's typed errors onto the SDK's exception types
- [ ] 2.5 Delete the superseded framing, socket and reconnect code

## 3. Push path

- [ ] 3.1 Consume SUBSCRIBE push frames through the client's push hook in `synap_sdk/modules/pubsub.py`

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — `sdks/python/README.md` and `CHANGELOG.md` (Unreleased → Changed)
- [ ] 4.2 Write tests covering the new behavior — keep `tests/test_transport.py` and `tests/test_rpc_parity_s2s.py` green and add an over-cap length-prefix test
- [ ] 4.3 Run tests and confirm they pass — lint plus `pytest`
