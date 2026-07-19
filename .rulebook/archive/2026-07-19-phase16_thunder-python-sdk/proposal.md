# Proposal: phase16_thunder-python-sdk

Source: https://github.com/hivellm/thunder — `docs/analysis/04-adoption-plan.md` (P3).

## Why

`sdks/python/synap_sdk/transport_rpc.py` is a hand-ported copy of the same ~600-line
wire layer that now exists 18 times across the family. It carries the drift risk and
the missing-cap class of bug Thunder was built to end, and every future wire-level
fix has to be re-applied here by hand.

`hivellm-thunder` 0.1.1 is published on PyPI (import name `thunder_rpc`) and ships
both a sync and an async client with the family's uniform floor: demux by id, frame
cap on encode and decode, connect + per-call timeouts, lazy reconnect, push hook,
typed error parsing.

## What Changes

- `sdks/python/pyproject.toml` adds `hivellm-thunder` and drops the direct `msgpack`
  dependency the local transport used.
- `synap_sdk/transport_rpc.py` becomes an adapter over `thunder_rpc.Client`, built
  from the Synap `Config` (scheme `synap`, port 15501, `AuthCommand` handshake,
  push enabled).
- Value conversion between the SDK's Python types and `thunder_rpc.Value` is
  centralized; `Bytes` decoding accepts both MessagePack `bin` and the legacy
  int-array form.
- `synap_sdk/modules/pubsub.py` consumes push frames from the client's push hook.
- `command_map.py` and the public `SynapClient` surface are untouched.

## Impact
- Affected specs: `.rulebook/tasks/phase16_thunder-python-sdk/specs/python-sdk-transport/spec.md`
- Affected code: `sdks/python/synap_sdk/transport_rpc.py`, `sdks/python/synap_sdk/modules/pubsub.py`, `sdks/python/pyproject.toml`
- Breaking change: NO — the public client API is unchanged.
- User benefit: enforced frame cap and timeouts, pipelined requests, and a wire
  layer maintained once for the whole family.
