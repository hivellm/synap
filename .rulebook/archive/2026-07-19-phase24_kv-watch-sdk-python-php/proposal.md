# Proposal: phase24_kv-watch-sdk-python-php

Source: docs/analysis/kv-watch-observable/ (F-009, F-010)

## Why

Watch parity across all SDKs: Python and PHP already have the push-subscribe primitive
(`transport_rpc.py` + `modules/pubsub.py`; `SynapRpcTransport.php` + `Module/PubSubManager.php`)
but no key-watch surface. The Rust/TS implementations from phase 23 define the reference
envelope decoding and API semantics to mirror.

## What Changes

- Python SDK: `kv.watch(pattern, mode="value")` returning an async iterator
  (`async for event in ...`), mirroring the pub/sub module's style. `WatchEvent` dataclass
  with key, event, version, value, truncated. Iterator close issues UNWATCH.
- PHP SDK: `KVManager::watch(string $pattern, callable $onEvent, string $mode = 'value')`
  plus an iterator variant, mirroring `PubSubManager`'s consumption style. `WatchEvent`
  value object. Explicit `unwatch()` support.
- Both: envelope decode matches the phase-23 reference; reconnect follows each SDK's
  existing pub/sub semantics.

## Impact

- Affected specs: specs/kv-watch-sdk/spec.md (ADDED Python/PHP requirements)
- Affected code: sdks/python/synap_sdk/ (kv module + new watch support),
  sdks/php/src/ (Module/KVManager or equivalent + WatchEvent)
- Breaking change: NO (additive API)
- User benefit: idiomatic key observation in Python (async iterator) and PHP (callback/iterator).
