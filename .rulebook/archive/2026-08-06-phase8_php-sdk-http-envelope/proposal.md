# Proposal: phase8_php-sdk-http-envelope

## Why

The PHP SDK's HTTP transport returns the raw StreamableHTTP envelope
(`{success, request_id, payload, error}`) from `SynapClient::execute()`, while
the native transports return the payload already normalised by `mapResponse()`.
Modules that read their fields directly therefore read them off the envelope and
find nothing. Measured against a live 1.3.1 server:

```
raw execute(kv.get):  {"success":true,"request_id":"…","payload":"v1","error":null}
kv()->get():          NULL          # over SynapRPC the same call returns 'v1'
hash()->get():        NULL
```

So on HTTP — the transport a plain `new SynapConfig('http://…')` selects —
`KVStore::get`, `exists`, `incr`, `decr`, `stats`, `scan`, the whole
`HashManager`, `ListManager`, `SetManager`, `StreamManager` and `QueueManager`
read surface silently returns the empty value instead of the data. Writes work
(the server applies them), so the failure looks like data loss rather than a
client bug. Twenty-nine call sites already work around it with
`$response['payload'] ?? $response`, which is how the bug survived: the modules
that got the workaround are fine, the ones that never got it are not.

This is not a 1.3.x regression — 1.2.3 behaves the same way — and fixing it
properly is not a one-liner: the payload shape differs per command between the
two paths (`kv.get` yields the bare string `"v1"` over HTTP and `['value' =>
'v1']` after `mapResponse` natively), so the normalisation has to be made
per-command rather than by unwrapping one level. That is why it is a task of its
own rather than a rider on the 1.3.1 sync.

## What Changes

- One normalisation point for HTTP replies, mirroring what `mapResponse()` does
  for the native wires, so both paths hand modules the same shape.
- The four call sites reading `$response['success']`
  (`HashManager::set`/`mset`, `ListManager::set`/`trim`) rechecked against the
  real payloads, since they currently read the envelope's always-true flag.
- The 29 `$response['payload'] ?? $response` workarounds collapsed once the
  client is consistent.
- S2S coverage that runs the read surface over HTTP, SynapRPC and RESP3 — the
  existing suite pins `http://localhost:15500` in `phpunit.xml`, so a
  transport-specific hole cannot be seen today.

## Impact

- Affected specs: none in this repository (the code lives in
  `hivellm/synap-sdk-php`)
- Affected code: `src/SynapClient.php`, every module under `src/Module/`,
  `tests/Unit/Module/*S2STest.php`, `phpunit.xml`
- Breaking change: NO for correct callers — a method that returned `null`
  starts returning the value it always documented. A caller that hard-coded the
  envelope shape (`$response['payload']`) keeps working through the fallback.
- User benefit: the PHP SDK reads data over HTTP at all.
