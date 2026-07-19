## 1. Dependency swap

- [x] 1.1 Add `@hivehub/thunder` to `sdks/typescript/package.json` and move `msgpackr` to devDependencies (still used by the byte-level loopback tests, no longer by the transport)
- [x] 1.2 Add a `synapConfig()` exporting the Synap `Config` (scheme `synap`, port 15501, `auth_command`, push enabled, `resp3_prefixes`, 512 MiB cap)
- [x] 1.3 `npx tsc --noEmit` clean

## 2. Transport rewrite

- [x] 2.1 Rewrite `src/transports/synap-rpc.ts` as an adapter over Thunder's `Client`
- [x] 2.2 Re-point `WireValue` to Thunder's `Value` and rewrite `toWireValue`/`fromWireValue` over it — `Bytes` decoding is Thunder's, covering both `bin` and the legacy int-array form, and `Int` now stays a `bigint` beyond ±2^53 instead of silently losing precision
- [x] 2.3 Route credentials through Thunder's client options instead of a hand-written AUTH frame
- [x] 2.4 Map Thunder's typed errors onto the SDK's error classes — Thunder classifies per `resp3_prefixes`, so `NOAUTH`/`WRONGPASS`/`NOPERM` arrive as `AuthError`
- [x] 2.5 Delete the superseded framing, socket and reconnect code

## 3. Push path

- [x] 3.1 Consume SUBSCRIBE push frames through Thunder's push hook, registered *before* the command is sent — replacing the previous `setTimeout(50)` guess at when the acknowledgement had landed

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [x] 4.1 Update or create documentation covering the implementation — `sdks/typescript/README.md` (new "The `synap://` transport is Thunder" section) and `CHANGELOG.md` (Changed/Added/Security)
- [x] 4.2 Write tests covering the new behavior — the existing byte-level loopback suite passes unchanged against Thunder's encoding, plus a new test asserting an over-cap length prefix is refused without allocating the claimed body
- [x] 4.3 Run tests and confirm they pass — `npm run test:unit` green (369 passed, 22 skipped) and `npm run build` clean. `npm run lint` reports 21 errors, all pre-existing in files this task did not touch (`command-map.ts`, `hash.ts`, `kv.ts`, s2s tests); the three files changed here are error-free.
