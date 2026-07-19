// Interop cell: TypeScript SDK (@hivehub/thunder) against a Thunder-based server.
//
// Driven by scripts/interop/run-matrix.py. Prints one
// `STEP <name> PASS|FAIL <detail>` line per step, exits non-zero on any failure.
//
// Runs against the SDK's built `dist/`, i.e. what a consumer installs, rather
// than against the TypeScript sources.

import { SynapClient } from '../../../../sdks/typescript/dist/index.js';

// Not valid UTF-8, so a transport that quietly round-trips through a string
// cannot pass the binary step.
const BINARY = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
const TOPIC = 'interop.typescript';

let failures = 0;

function report(step, ok, detail) {
  console.log(`STEP ${step} ${ok ? 'PASS' : 'FAIL'} ${detail}`);
  if (!ok) failures += 1;
}

const [host, port, user, pass] = process.argv.slice(2);

const client = new SynapClient({
  url: `synap://${host}:${port}`,
  timeout: 15000,
  auth: { type: 'basic', username: user, password: pass },
});

const rpc = client.synapRpcTransport();
if (!rpc) {
  report('auth', false, 'synap:// URL did not select the SynapRPC transport');
  process.exit(1);
}

// 1. Authenticate. Credentials ride the handshake on the first call; the
//    pre-Thunder transport never sent AUTH, so a require_auth server was
//    unreachable.
//
//    EXISTS rather than PING: the server answers PING before authentication,
//    so a PING probe passes just as happily on a connection that never
//    authenticated -- which is exactly the bug this column exists to catch.
try {
  const probe = await rpc.execute('EXISTS', ['interop:ts:probe']);
  report('auth', true, `EXISTS -> ${JSON.stringify(probe)}`);
} catch (err) {
  report('auth', false, `${err.constructor.name}: ${err.message}`);
  process.exit(1);
}

// 2. SET/GET a binary value — canonical MessagePack bin, byte-exact back.
try {
  await rpc.execute('SET', ['interop:ts:bin', BINARY]);
  const got = await rpc.execute('GET', ['interop:ts:bin']);
  const bytes = Buffer.isBuffer(got) ? got : Buffer.from(got, 'binary');
  report('kv_binary', bytes.equals(BINARY), `${BINARY.toString('hex')} -> ${bytes.toString('hex')}`);
} catch (err) {
  report('kv_binary', false, `${err.constructor.name}: ${err.message}`);
}

// 3. SUBSCRIBE then PUBLISH — the push frame must arrive on the hook.
let subscription;
try {
  const received = [];
  const arrived = new Promise((resolve) => {
    subscription = rpc.subscribePush([TOPIC], (msg) => {
      received.push(msg);
      resolve();
    });
  });
  subscription = await subscription;

  await rpc.execute('PUBLISH', [TOPIC, 'interop-payload']);
  await Promise.race([arrived, new Promise((r) => setTimeout(r, 10000))]);

  const ok = received.length > 0 && received[0].topic === TOPIC;
  report('pubsub', ok, `received=${JSON.stringify(received.slice(0, 1))}`);
} catch (err) {
  report('pubsub', false, `${err.constructor.name}: ${err.message}`);
} finally {
  subscription?.cancel?.();
}

// 4. Error round-trip — an unknown command must reject, and must not poison
//    the multiplexed connection.
try {
  const result = await rpc.execute('NOSUCHCOMMAND', []);
  report('error', false, `expected a rejection, got ${JSON.stringify(result)}`);
} catch (err) {
  const alive = (await rpc.execute('PING', [])) === 'PONG';
  report('error', alive, `rejected with ${err.constructor.name}; connection alive=${alive}`);
}

rpc.close();
client.close?.();
process.exit(failures ? 1 : 0);
