# Proposal: phase6_csharp-http-command-names

Source: manual release validation (2026-07-21) — smoke harnesses and SDK S2S
suites run against a live 1.3.0 release server, which the hard-skipped/mocked
SDK test suites had never done.

## Why

The cross-SDK ↔ server compatibility matrix had real holes that only surface
against a live server:

1. **C# SDK, HTTP transport (default)**: sent `kv.delete`/`hash.values`
   (server knows `kv.del`/`hash.vals`) and returned the raw command envelope
   to modules, so **every read returned empty/default silently**. Its S2S
   tests are hard-skipped (`[Fact(Skip=...)]`), so this shipped unnoticed.
2. **C# SDK, native transports**: dead mapper keys (`hash.delete`,
   `hash.incr` with the wrong payload key) made HDEL/HINCRBY throw
   `UnsupportedCommandException`; SynapRPC lists decode to `List<object?>`
   which `RawToList` silently dropped (HVALS etc. returned `[]`).
3. **Server command endpoint**: `hash.del` required a `fields` array while
   every SDK (TS/Python/C#) sends a singular `field`.
4. **Server native dispatchers**: `HINCRBY`/`HINCRBYFLOAT` missing from both
   RESP3 and SynapRPC (all three SDKs map `hash.incrby` to them); the whole
   stream family (`SCREATE`/`SGETORCREATE`/`SPUBLISH`/`SREAD`/`SDELETE`/
   `SLIST`/`SSTATS`) missing from RESP3 (TS and Python map `stream.*` to it).
5. **Python SDK**: queue publish sent the raw object instead of the byte-list
   wire shape (broken on every transport); consume didn't decode payloads nor
   the flat native reply shapes; `hash.incrby` native mapping read `amount`
   while modules send `increment` (increment silently became 1); stale S2S
   tests used long-removed APIs.
6. **Transactional writes over native transports silently bypass MULTI**
   (raw wire commands carry no client_id). The Python SDK now refuses the
   mapping explicitly (`UnsupportedCommandError`) instead of corrupting;
   proper native queuing is a follow-up.

## What Changes

- C# SDK: correct command names, payload envelope unwrapping (object/array
  and scalar payloads), live mapper keys, `AsRawArray` accepting both list
  shapes, `hash.del` response shaping; mapper tests updated.
- Server: `hash.del` command accepts `field` or `fields`; RESP3 + SynapRPC
  gain HINCRBY/HINCRBYFLOAT; RESP3 gains the stream family (mirroring
  SynapRPC), SCREATE accepting the SDKs' optional max_events arg.
- Python SDK: queue payload byte-list encode/decode symmetric with TS;
  native QCONSUME reply normalization (map and array forms); `increment`
  payload key; client_id guard refusing native transactional writes; stale
  tests fixed.
- Regression tests: RESP3 + RPC dispatch tests for HINCRBY/HINCRBYFLOAT,
  command-endpoint test for `hash.del` field/fields.

## Impact
- Affected specs: none (bug fixes restoring the documented contracts)
- Affected code: `sdks/csharp/src`, `sdks/python/synap_sdk`, server RESP3 +
  SynapRPC dispatchers, `handlers/hash.rs`, tests in all three
- Breaking change: NO public API change. Python native transactional writes
  now raise `UnsupportedCommandError` instead of silently executing outside
  the transaction — strictly safer.
- User benefit: the three transports behave identically for KV/hash/queue/
  stream across TS, Python and C#, verified end-to-end against a live server
