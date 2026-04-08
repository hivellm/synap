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
import { Synap } from '../index';

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
  return new Synap({
    url: `http://127.0.0.1:${HTTP_PORT}`,
    transport: 'http',
    timeout: 5000,
  });
}

function rpcClient(): Synap {
  return new Synap({
    url: `http://127.0.0.1:${HTTP_PORT}`,
    transport: 'synaprpc',
    rpcHost: '127.0.0.1',
    rpcPort: RPC_PORT,
    timeout: 5000,
  });
}

function resp3Client(): Synap {
  return new Synap({
    url: `http://127.0.0.1:${HTTP_PORT}`,
    transport: 'resp3',
    resp3Host: '127.0.0.1',
    resp3Port: RESP3_PORT,
    timeout: 5000,
  });
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
