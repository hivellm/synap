/**
 * Starts a real synap-server for the GUI contract and smoke tests.
 *
 * The binary is `SYNAP_SERVER_BIN` when set, otherwise the newest of
 * `target/debug` and `target/release` at the repository root. Build it first:
 *
 *     cargo build -p synap-server --bin synap-server
 *
 * Each server gets a free port, a temporary working directory (its WAL and
 * snapshots land there) and a configuration with authentication required,
 * a known root password and a Hub service key, so the tests can check that
 * secrets never come back. Two API keys are minted through the real
 * `/auth/keys` endpoint: one holding every permission (admin) and one that
 * may only read.
 */
import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { request } from 'node:http';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SynapApiClient } from '@/services/api';

export const ROOT_USER = 'root';
export const ROOT_PASSWORD = 'Contract-Root-Pass-7f3a9c!';
export const HUB_SERVICE_KEY = 'contract-hub-service-key-91c2d4';

const HOST = '127.0.0.1';
// Resolved by hand: vite rewrites `new URL(path, import.meta.url)` into an
// asset import in DOM test environments.
const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const STARTUP_TIMEOUT_MS = 60_000;

export interface TestServer {
  /** Base URL without the port, as the GUI stores it. */
  url: string;
  port: number;
  /** API key holding every permission, `admin:*` included. */
  adminKey: string;
  /** API key that may only read. */
  readOnlyKey: string;
  /** A GUI API client for this server, authenticated with `apiKey` when given. */
  client(apiKey?: string): SynapApiClient;
  stop(): Promise<void>;
}

function serverBinary(): string {
  const fromEnv = process.env.SYNAP_SERVER_BIN;
  if (fromEnv) {
    if (!existsSync(fromEnv)) {
      throw new Error(`SYNAP_SERVER_BIN points to a missing file: ${fromEnv}`);
    }
    return fromEnv;
  }
  const exe = process.platform === 'win32' ? 'synap-server.exe' : 'synap-server';
  const candidates = ['debug', 'release']
    .map((profile) => resolve(REPO_ROOT, 'target', profile, exe))
    .filter((path) => existsSync(path))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (candidates.length === 0) {
    throw new Error(
      'synap-server binary not found. Build it first with ' +
        '`cargo build -p synap-server --bin synap-server`, or set SYNAP_SERVER_BIN.',
    );
  }
  return candidates[0];
}

function freePort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const probe = createServer();
    probe.unref();
    probe.on('error', reject);
    probe.listen(0, HOST, () => {
      const address = probe.address();
      const port = typeof address === 'object' && address ? address.port : 0;
      probe.close(() => resolvePort(port));
    });
  });
}

function configYaml(port: number): string {
  return `server:
  host: "${HOST}"
  port: ${port}
  websocket_enabled: false
kv_store:
  max_memory_mb: 256
  eviction_policy: "allkeys-lru"
  ttl_cleanup_interval_ms: 100
queue:
  enabled: true
  max_depth: 10000
  ack_deadline_secs: 30
  default_max_retries: 3
  default_priority: 5
rate_limit:
  enabled: false
  requests_per_second: 1000
  burst_size: 100
logging:
  level: "warn"
  format: "pretty"
protocols:
  streamable_http:
    enabled: true
    path: "/api/v1/command"
  rest:
    enabled: true
    prefix: "/kv"
auth:
  enabled: true
  require_auth: true
  root:
    username: "${ROOT_USER}"
    password: "${ROOT_PASSWORD}"
    enabled: true
hub:
  enabled: false
  service_api_key: "${HUB_SERVICE_KEY}"
resp3:
  enabled: false
synap_rpc:
  enabled: false
`;
}

/** The environment without SYNAP_* overrides, so only the config file decides. */
function cleanEnv(): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {};
  for (const [name, value] of Object.entries(process.env)) {
    if (!name.startsWith('SYNAP_')) env[name] = value;
  }
  env.RUST_LOG = 'warn';
  return env;
}

/**
 * One HTTP exchange through node:http rather than the global fetch, which a
 * DOM test environment replaces with its own (that one logs every refused
 * connection while the server is still starting).
 */
function httpJson(
  method: string,
  url: string,
  headers: Record<string, string> = {},
  body?: unknown,
): Promise<{ status: number; body: unknown }> {
  return new Promise((resolveExchange, reject) => {
    const payload = body === undefined ? undefined : JSON.stringify(body);
    const req = request(
      url,
      {
        method,
        headers: payload === undefined ? headers : { ...headers, 'Content-Type': 'application/json' },
      },
      (res) => {
        let text = '';
        res.setEncoding('utf-8');
        res.on('data', (chunk: string) => (text += chunk));
        res.on('end', () => {
          let parsed: unknown = text;
          try {
            parsed = JSON.parse(text);
          } catch {
            // Not JSON: keep the text.
          }
          resolveExchange({ status: res.statusCode ?? 0, body: parsed });
        });
      },
    );
    req.on('error', reject);
    req.end(payload);
  });
}

async function waitForHealth(base: string, child: ChildProcess, output: () => string): Promise<void> {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`synap-server exited with code ${child.exitCode}:\n${output()}`);
    }
    try {
      const response = await httpJson('GET', `${base}/health`);
      if (response.status === 200) return;
    } catch {
      // Not listening yet.
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`synap-server did not become healthy within ${STARTUP_TIMEOUT_MS} ms:\n${output()}`);
}

async function createApiKey(
  base: string,
  name: string,
  permissions: { resource: string; action: string }[],
): Promise<string> {
  const basic = Buffer.from(`${ROOT_USER}:${ROOT_PASSWORD}`).toString('base64');
  const response = await httpJson(
    'POST',
    `${base}/auth/keys`,
    { Authorization: `Basic ${basic}` },
    { name, permissions },
  );
  const body = response.body as { key?: { key?: unknown } } | null;
  const key = body?.key?.key;
  if (response.status !== 200 || typeof key !== 'string') {
    throw new Error(`cannot create API key ${name}: HTTP ${response.status} ${JSON.stringify(response.body)}`);
  }
  return key;
}

function stopChild(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null) return Promise.resolve();
  return new Promise((resolveStop) => {
    child.once('exit', () => resolveStop());
    child.kill();
  });
}

/** Start a fresh synap-server and mint its admin and read-only API keys. */
export async function startTestServer(): Promise<TestServer> {
  const binary = serverBinary();
  const port = await freePort();
  const dir = await mkdtemp(join(tmpdir(), 'synap-gui-contract-'));
  const configPath = join(dir, 'config.yml');
  await writeFile(configPath, configYaml(port), 'utf-8');

  let log = '';
  const child = spawn(binary, ['--config', configPath], {
    cwd: dir,
    env: cleanEnv(),
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const capture = (chunk: Buffer): void => {
    log = (log + chunk.toString()).slice(-20_000);
  };
  child.stdout?.on('data', capture);
  child.stderr?.on('data', capture);

  const url = `http://${HOST}`;
  const base = `${url}:${port}`;
  const stop = async (): Promise<void> => {
    await stopChild(child);
    await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  };

  try {
    await waitForHealth(base, child, () => log);
    const adminKey = await createApiKey(base, 'gui-contract-admin', [{ resource: '*', action: 'all' }]);
    const readOnlyKey = await createApiKey(base, 'gui-contract-reader', [{ resource: '*', action: 'read' }]);
    return {
      url,
      port,
      adminKey,
      readOnlyKey,
      client: (apiKey?: string) => new SynapApiClient({ name: 'contract', url, port, apiKey }),
      stop,
    };
  } catch (error) {
    await stop();
    throw error;
  }
}
