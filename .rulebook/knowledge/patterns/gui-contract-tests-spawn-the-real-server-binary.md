# GUI contract tests spawn the real server binary

**Category**: testing
**Tags**: gui, vitest, contract-tests, synap-server, phase27

## Description

The GUI called envelope commands the server never defined (`admin.config`, `kv.type`, `kv.memory`) and REST routes that do not exist, and every view hid the failure behind a fallback (sample config, cleared inspector). Mocked API tests would have encoded the same wrong names. The fix that catches this class of drift: Vitest tests that start the real `synap-server` binary per test file (free port, temp cwd, auth required, known secrets in the config), mint API keys through the real `/auth/keys` endpoint, and drive `src/services/api.ts` (contract) and mounted views via `@vue/test-utils` + happy-dom (smoke) against it.

Details that matter:
- Use `node:http` in the harness, not global `fetch`: a DOM environment swaps `fetch` for its own, which logs every refused connection while the server boots.
- Do not build the binary path with `new URL(path, import.meta.url)`: in a DOM environment Vite rewrites it into an asset import (`Denied ID ...?url`). Use `fileURLToPath(import.meta.url)` + `path.resolve`.
- Seed order-sensitive structures (lists) sequentially; `Promise.all` pushes arrive in any order and make the test flaky.
- Put secrets in the spawned config and assert they never appear in any response.

## Example

gui/tests/support/server.ts (startTestServer), gui/tests/contract/api-client.contract.test.ts, gui/tests/smoke/structures.smoke.test.ts. Run: `cargo build -p synap-server --bin synap-server` then `npm test` in gui/.

## When to Use

Any GUI or SDK client code whose correctness depends on server command names or response shapes.

## When NOT to Use

Pure rendering logic with no server contract.
