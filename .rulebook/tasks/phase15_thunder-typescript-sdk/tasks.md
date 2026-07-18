## 1. Dependency swap

- [ ] 1.1 Add `@hivehub/thunder` to `sdks/typescript/package.json` and remove the transport's direct msgpack dependency
- [ ] 1.2 Add a `synapConfig` module exporting the Synap `Config` (scheme `synap`, port 15501, `AuthCommand`, push enabled)
- [ ] 1.3 `npm run type-check` clean

## 2. Transport rewrite

- [ ] 2.1 Rewrite `src/transports/synap-rpc.ts` as an adapter over Thunder's `Client`
- [ ] 2.2 Centralize SDK-value ↔ `thunder.Value` conversion, decoding `Bytes` from both `bin` and the legacy int-array form
- [ ] 2.3 Route credentials through Thunder's client options instead of a hand-written AUTH frame
- [ ] 2.4 Map Thunder's typed errors onto the SDK's error classes
- [ ] 2.5 Delete the superseded framing, socket and reconnect code

## 3. Push path

- [ ] 3.1 Consume SUBSCRIBE push frames through Thunder's push hook in `src/pubsub.ts`

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — `sdks/typescript/README.md` and `CHANGELOG.md` (Unreleased → Changed)
- [ ] 4.2 Write tests covering the new behavior — keep `src/__tests__/transport.unit.test.ts` green and add a test asserting an over-cap length prefix is refused without allocating
- [ ] 4.3 Run tests and confirm they pass — `npm run lint && npm test`
