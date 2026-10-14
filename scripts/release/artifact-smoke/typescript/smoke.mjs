// Artifact smoke test for the TypeScript SDK.
//
// Run by scripts/release/artifact-smoke/run.py from a scratch project that
// installed the packed tarball, so `@hivehub/synap` resolves to the artifact
// that is about to be published, not to the source tree.
//
//   argv:   <http-url> <synap-url> <resp3-url>
//   stdout: one `STEP <transport>.<name> PASS|FAIL <detail>` line per step
//   exit:   0 if every step passed
import { Synap } from '@hivehub/synap';

const [httpUrl, rpcUrl, resp3Url] = process.argv.slice(2);
if (!httpUrl || !rpcUrl || !resp3Url) {
  console.error('usage: smoke.mjs <http-url> <synap-url> <resp3-url>');
  process.exit(2);
}

let failures = 0;
function report(step, ok, detail) {
  console.log(`STEP ${step} ${ok ? 'PASS' : 'FAIL'} ${detail}`);
  if (!ok) failures += 1;
}

async function check(step, run) {
  try {
    const [ok, detail] = await run();
    report(step, ok, detail);
  } catch (error) {
    report(step, false, `threw ${error?.name ?? 'error'}: ${error?.message ?? error}`);
  }
}

async function transportSuite(name, url) {
  const synap = new Synap({ url, timeout: 5000 });
  const key = `smoke:ts:${name}:key`;
  const counter = `smoke:ts:${name}:counter`;
  try {
    await check(`${name}.set_get`, async () => {
      await synap.kv.set(key, `value-${name}`);
      const got = await synap.kv.get(key);
      return [got === `value-${name}`, `got ${JSON.stringify(got)}`];
    });
    await check(`${name}.counter_amounts`, async () => {
      await synap.kv.set(counter, '0');
      const seen = [
        await synap.kv.incr(counter, 10),
        await synap.kv.decr(counter, 4),
        await synap.kv.incr(counter),
      ];
      return [JSON.stringify(seen) === '[10,6,7]', `incr 10, decr 4, incr -> ${JSON.stringify(seen)}`];
    });
    await check(`${name}.delete`, async () => {
      await synap.kv.del(key);
      await synap.kv.del(counter);
      const got = await synap.kv.get(key);
      return [got === null, `get after del -> ${JSON.stringify(got)}`];
    });
  } finally {
    synap.close();
  }
}

await transportSuite('http', httpUrl);
await transportSuite('synaprpc', rpcUrl);
await transportSuite('resp3', resp3Url);

const http = new Synap({ url: httpUrl, timeout: 5000 });
const rpc = new Synap({ url: rpcUrl, timeout: 5000 });
const resp3 = new Synap({ url: resp3Url, timeout: 5000 });
try {
  await check('cross.http_to_native', async () => {
    const key = 'smoke:ts:cross';
    await http.kv.set(key, 'from-http');
    const seen = [await rpc.kv.get(key), await resp3.kv.get(key)];
    await http.kv.del(key);
    return [seen.every((v) => v === 'from-http'), `synaprpc, resp3 -> ${JSON.stringify(seen)}`];
  });
} finally {
  http.close();
  rpc.close();
  resp3.close();
}

// exitCode rather than process.exit(): exiting while the closed clients' sockets
// are still shutting down aborts Node on Windows (libuv UV_HANDLE_CLOSING).
process.exitCode = failures === 0 ? 0 : 1;
