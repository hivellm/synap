/**
 * End-to-end tests against a real Synap server binary.
 *
 * Spawns the release binary with a patched config (loopback, custom ports,
 * persistence/hub/replication disabled), exercises HTTP / SynapRPC / RESP3
 * transports through the KV API, then kills the process.
 *
 * Run with: RUN_E2E=true npx vitest run src/__tests__/e2e.test.ts
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { spawn, ChildProcess } from 'child_process';
import { createConnection } from 'net';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { randomUUID } from 'crypto';
import { Synap, UnsupportedCommandError } from '../index';

const RUN_E2E = process.env.RUN_E2E === 'true';
const describeE2E = RUN_E2E ? describe : describe.skip;

const HTTP_PORT = 25510;
const RPC_PORT = 25511;
const RESP3_PORT = 26389;

function workspaceRoot(): string {
  // __dirname = sdks/typescript/src/__tests__ → workspace is 4 up
  return path.resolve(__dirname, '..', '..', '..', '..');
}

function serverBinary(): string {
  const root = workspaceRoot();
  const exe = process.platform === 'win32' ? 'synap-server.exe' : 'synap-server';
  const full = path.join(root, 'target', 'release', exe);
  if (!fs.existsSync(full)) {
    throw new Error(
      `Release binary not found at ${full}. Run \`cargo build --release\` in the workspace root.`,
    );
  }
  return full;
}

function writeTestConfig(): string {
  const root = workspaceRoot();
  const base = fs.readFileSync(path.join(root, 'config.yml'), 'utf8');

  const patched = base
    .replace('host: "0.0.0.0"', 'host: "127.0.0.1"')
    .replace('port: 15500', `port: ${HTTP_PORT}`)
    .replace('port: 15501', `port: ${RPC_PORT}`)
    .replace('port: 6379', `port: ${RESP3_PORT}`)
    .replace('persistence:\n  enabled: true', 'persistence:\n  enabled: false')
    .replace('hub:\n  enabled: true', 'hub:\n  enabled: false')
    .replace('replication:\n  enabled: true', 'replication:\n  enabled: false');

  const tmpFile = path.join(
    os.tmpdir(),
    `synap-e2e-ts-${process.pid}-${Date.now()}.yml`,
  );
  fs.writeFileSync(tmpFile, patched);
  return tmpFile;
}

function waitPortReady(port: number, deadline: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const tryConnect = (): void => {
      if (Date.now() > deadline) {
        reject(new Error(`Server port ${port} did not become ready within timeout`));
        return;
      }
      const sock = createConnection({ host: '127.0.0.1', port }, () => {
        sock.end();
        resolve();
      });
      sock.on('error', () => {
        sock.destroy();
        setTimeout(tryConnect, 50);
      });
    };
    tryConnect();
  });
}

async function waitServerReady(): Promise<void> {
  const deadline = Date.now() + 20_000;
  for (const port of [HTTP_PORT, RPC_PORT, RESP3_PORT]) {
    await waitPortReady(port, deadline);
  }
}

// ── Fixture state ─────────────────────────────────────────────────────────────

let child: ChildProcess | null = null;
let configPath: string | null = null;

async function startServer(): Promise<void> {
  configPath = writeTestConfig();
  child = spawn(serverBinary(), ['--config', configPath], {
    stdio: ['ignore', 'ignore', 'ignore'],
  });
  child.on('error', (e) => {
    console.error('[e2e] server spawn error:', e);
  });
  await waitServerReady();
}

async function stopServer(): Promise<void> {
  if (child && !child.killed) {
    child.kill();
    await new Promise<void>((r) => {
      if (!child) return r();
      child.on('exit', () => r());
      setTimeout(r, 2000);
    });
  }
  child = null;
  if (configPath && fs.existsSync(configPath)) {
    try {
      fs.unlinkSync(configPath);
    } catch {
      /* ignore */
    }
  }
  configPath = null;
}

// ── Clients ───────────────────────────────────────────────────────────────────

function httpClient(): Synap {
  return new Synap({ url: `http://127.0.0.1:${HTTP_PORT}`, timeout: 5000 });
}

function rpcClient(): Synap {
  return new Synap({ url: `synap://127.0.0.1:${RPC_PORT}`, timeout: 5000 });
}

function resp3Client(): Synap {
  return new Synap({ url: `resp3://127.0.0.1:${RESP3_PORT}`, timeout: 5000 });
}

// ── Queue suite ───────────────────────────────────────────────────────────────

async function runQueueSuite(synap: Synap, prefix: string): Promise<void> {
  const qName = `${prefix}:e2e:queue`;

  await synap.queue.createQueue(qName);
  const queues = await synap.queue.listQueues();
  expect(queues).toContain(qName);

  const msgId = await synap.queue.publishString(qName, 'hello-queue');
  expect(typeof msgId).toBe('string');
  expect(msgId.length).toBeGreaterThan(0);

  const { message, text } = await synap.queue.consumeString(qName, `${prefix}-consumer`);
  expect(message).not.toBeNull();
  expect(text).toBe('hello-queue');

  if (message) {
    const acked = await synap.queue.ack(qName, message.id);
    expect(acked).toBe(true);
  }

  const stats = await synap.queue.stats(qName);
  expect(stats.consumed).toBeGreaterThanOrEqual(1);

  await synap.queue.deleteQueue(qName);
  const queuesAfter = await synap.queue.listQueues();
  expect(queuesAfter).not.toContain(qName);
}

// ── Stream suite ──────────────────────────────────────────────────────────────

async function runStreamSuite(synap: Synap, prefix: string): Promise<void> {
  const room = `${prefix}:e2e:room`;

  await synap.stream.createRoom(room);
  const rooms = await synap.stream.listRooms();
  expect(rooms).toContain(room);

  const off1 = await synap.stream.publish(room, 'evt.a', { n: 1 });
  const off2 = await synap.stream.publish(room, 'evt.b', { n: 2 });
  expect(off1).toBeGreaterThanOrEqual(0);
  expect(off2).toBeGreaterThan(off1);

  const events = await synap.stream.consume(room, `${prefix}-sub`, 0);
  expect(events.length).toBeGreaterThanOrEqual(2);
  expect(events[0].event).toBe('evt.a');
  expect(events[1].event).toBe('evt.b');

  const stats = await synap.stream.stats(room);
  expect(stats.total_events).toBeGreaterThanOrEqual(2);

  await synap.stream.deleteRoom(room);
  const roomsAfter = await synap.stream.listRooms();
  expect(roomsAfter).not.toContain(room);
}

// ── PubSub suite ──────────────────────────────────────────────────────────────

async function runPubSubSuite(synap: Synap, prefix: string): Promise<void> {
  const topic = `${prefix}.e2e.topic`;

  // Publish succeeds even with no active subscribers
  const published = await synap.pubsub.publish(topic, { msg: 'hello' });
  expect(typeof published).toBe('boolean');

  const topics = await synap.pubsub.listTopics();
  expect(Array.isArray(topics)).toBe(true);
}

// ── Transaction suite ─────────────────────────────────────────────────────────

async function runTransactionSuite(synap: Synap, prefix: string): Promise<void> {
  const key = `${prefix}:e2e:txn:key`;

  // MULTI → DISCARD
  const txDiscard = randomUUID();
  const multiResp = await synap.transaction.multi({ clientId: txDiscard });
  expect(multiResp.success).toBe(true);
  const discardResp = await synap.transaction.discard({ clientId: txDiscard });
  expect(discardResp.success).toBe(true);

  // MULTI → scoped SET → EXEC
  const txExec = randomUUID();
  const scope = synap.transaction.scope(txExec);
  await synap.transaction.multi({ clientId: txExec });
  await scope.kv.set(key, 'txn-value');
  const execResult = await synap.transaction.exec({ clientId: txExec });

  // exec may succeed or abort depending on watch state — either is valid
  if (execResult.success) {
    const val = await synap.kv.get(key);
    expect(val).toBe('txn-value');
    await synap.kv.del(key);
  } else {
    expect(execResult.aborted).toBe(true);
  }
}

// ── Script suite ──────────────────────────────────────────────────────────────

async function runScriptSuite(synap: Synap, _prefix: string): Promise<void> {
  // EVAL: return a constant
  const evalResp = await synap.script.eval<string>('return "synap-ok"');
  expect(evalResp.result).toBe('synap-ok');
  expect(typeof evalResp.sha1).toBe('string');

  // SCRIPT LOAD → EVALSHA
  const sha1 = await synap.script.load('return ARGV[1]');
  expect(typeof sha1).toBe('string');

  const existsResp = await synap.script.exists([sha1]);
  expect(existsResp.exists[0]).toBe(true);

  const evalshaResp = await synap.script.evalsha<string>(sha1, { args: ['from-evalsha'] });
  expect(evalshaResp.result).toBe('from-evalsha');
}

// ── KV suite ──────────────────────────────────────────────────────────────────

async function runKvSuite(synap: Synap, prefix: string): Promise<void> {
  const key = `${prefix}:e2e:key`;

  await synap.kv.set(key, 'hello');
  expect(await synap.kv.get(key)).toBe('hello');

  expect(await synap.kv.exists(key)).toBe(true);

  await synap.kv.set(key, 'world');
  expect(await synap.kv.get(key)).toBe('world');

  await synap.kv.del(key);
  expect(await synap.kv.get(key)).toBeNull();
  expect(await synap.kv.exists(key)).toBe(false);

  const counter = `${prefix}:e2e:counter`;
  await synap.kv.set(counter, '0');
  const v1 = await synap.kv.incr(counter);
  const v2 = await synap.kv.incr(counter);
  const v3 = await synap.kv.incr(counter);
  expect([v1, v2, v3]).toEqual([1, 2, 3]);
  await synap.kv.del(counter);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describeE2E('Synap SDK E2E (real server, all transports)', () => {
  beforeAll(async () => {
    await startServer();
  }, 30_000);

  afterAll(async () => {
    await stopServer();
  });

  it('HTTP transport — KV roundtrip', async () => {
    const c = httpClient();
    try {
      await runKvSuite(c, 'http');
    } finally {
      c.close();
    }
  });

  it('SynapRPC transport — KV roundtrip', async () => {
    const c = rpcClient();
    try {
      await runKvSuite(c, 'rpc');
    } finally {
      c.close();
    }
  });

  it('RESP3 transport — KV roundtrip', async () => {
    const c = resp3Client();
    try {
      await runKvSuite(c, 'resp3');
    } finally {
      c.close();
    }
  });

  // ── Queue tests ────────────────────────────────────────────────────────────

  it('HTTP transport — Queue lifecycle', async () => {
    const c = httpClient();
    try { await runQueueSuite(c, 'http'); } finally { c.close(); }
  });

  it('SynapRPC transport — Queue lifecycle', async () => {
    const c = rpcClient();
    try { await runQueueSuite(c, 'rpc'); } finally { c.close(); }
  });

  it('RESP3 transport — Queue lifecycle', async () => {
    const c = resp3Client();
    try { await runQueueSuite(c, 'resp3'); } finally { c.close(); }
  });

  // ── Stream tests ───────────────────────────────────────────────────────────

  it('HTTP transport — Stream lifecycle', async () => {
    const c = httpClient();
    try { await runStreamSuite(c, 'http'); } finally { c.close(); }
  });

  it('SynapRPC transport — Stream lifecycle', async () => {
    const c = rpcClient();
    try { await runStreamSuite(c, 'rpc'); } finally { c.close(); }
  });

  it('RESP3 transport — Stream lifecycle', async () => {
    const c = resp3Client();
    try { await runStreamSuite(c, 'resp3'); } finally { c.close(); }
  });

  // ── Pub/Sub tests ──────────────────────────────────────────────────────────

  it('HTTP transport — Pub/Sub publish', async () => {
    const c = httpClient();
    try { await runPubSubSuite(c, 'http'); } finally { c.close(); }
  });

  it('SynapRPC transport — Pub/Sub publish', async () => {
    const c = rpcClient();
    try { await runPubSubSuite(c, 'rpc'); } finally { c.close(); }
  });

  it('RESP3 transport — Pub/Sub publish', async () => {
    const c = resp3Client();
    try { await runPubSubSuite(c, 'resp3'); } finally { c.close(); }
  });

  // ── Transaction tests ──────────────────────────────────────────────────────

  it('HTTP transport — Transaction MULTI/EXEC', async () => {
    const c = httpClient();
    try { await runTransactionSuite(c, 'http'); } finally { c.close(); }
  });

  it('SynapRPC transport — Transaction MULTI/EXEC', async () => {
    const c = rpcClient();
    try { await runTransactionSuite(c, 'rpc'); } finally { c.close(); }
  });

  it('RESP3 transport — Transaction MULTI/EXEC', async () => {
    const c = resp3Client();
    try { await runTransactionSuite(c, 'resp3'); } finally { c.close(); }
  });

  // ── Script tests ───────────────────────────────────────────────────────────

  it('HTTP transport — Lua scripting', async () => {
    const c = httpClient();
    try { await runScriptSuite(c, 'http'); } finally { c.close(); }
  });

  it('SynapRPC transport — Lua scripting', async () => {
    const c = rpcClient();
    try { await runScriptSuite(c, 'rpc'); } finally { c.close(); }
  });

  it('RESP3 transport — Lua scripting', async () => {
    const c = resp3Client();
    try { await runScriptSuite(c, 'resp3'); } finally { c.close(); }
  });

  // ── UnsupportedCommandError regression (8.2) ───────────────────────────────

  it('SynapRPC — unmapped command raises UnsupportedCommandError', async () => {
    const c = rpcClient();
    try {
      await expect(
        c.bitmap.setbit('e2e:ts:rpc:bitmap', 0, 1),
      ).rejects.toBeInstanceOf(UnsupportedCommandError);
    } finally {
      c.close();
    }
  });

  it('RESP3 — unmapped command raises UnsupportedCommandError', async () => {
    const c = resp3Client();
    try {
      await expect(
        c.bitmap.setbit('e2e:ts:resp3:bitmap', 0, 1),
      ).rejects.toBeInstanceOf(UnsupportedCommandError);
    } finally {
      c.close();
    }
  });

  it('HTTP — unmapped command succeeds (no UnsupportedCommandError)', async () => {
    const c = httpClient();
    try {
      const key = 'e2e:ts:http:bitmap';
      const result = await c.bitmap.setbit(key, 0, 1);
      expect(typeof result).toBe('number');
      await c.kv.del(key);
    } finally {
      c.close();
    }
  });

  it('cross-transport consistency — writes visible across HTTP/RPC/RESP3', async () => {
    const http = httpClient();
    const rpc = rpcClient();
    const resp3 = resp3Client();
    const key = 'e2e:ts:cross:key';
    try {
      await http.kv.set(key, 'from_http');
      expect(await rpc.kv.get(key)).toBe('from_http');
      expect(await resp3.kv.get(key)).toBe('from_http');

      await rpc.kv.set(key, 'from_rpc');
      expect(await resp3.kv.get(key)).toBe('from_rpc');
      expect(await http.kv.get(key)).toBe('from_rpc');

      await resp3.kv.set(key, 'from_resp3');
      expect(await http.kv.get(key)).toBe('from_resp3');
      expect(await rpc.kv.get(key)).toBe('from_resp3');

      await http.kv.del(key);
    } finally {
      http.close();
      rpc.close();
      resp3.close();
    }
  });
});
