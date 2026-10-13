/**
 * Contract tests of the GUI API client (src/services/api.ts) against a real
 * synap-server: every call here goes over HTTP to the binary started by
 * tests/support/server.ts, so a renamed command or a changed response shape
 * fails these tests instead of silently breaking a view.
 */
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import type { SynapApiClient } from '@/services/api';
import { HUB_SERVICE_KEY, ROOT_PASSWORD, startTestServer, type TestServer } from '../support/server';

let server: TestServer;
let admin: SynapApiClient;
let reader: SynapApiClient;
let anonymous: SynapApiClient;

beforeAll(async () => {
  server = await startTestServer();
  admin = server.client(server.adminKey);
  reader = server.client(server.readOnlyKey);
  anonymous = server.client();
});

afterAll(async () => {
  await server?.stop();
});

describe('admin.config (configuration view)', () => {
  it('returns the effective configuration to an admin with every secret redacted', async () => {
    const response = await admin.getServerConfig();
    expect(response.success, response.error).toBe(true);
    const { config, yaml } = response.data!;

    const section = (name: string): Record<string, unknown> => config[name] as Record<string, unknown>;
    expect(section('server').port).toBe(server.port);
    expect(section('auth').require_auth).toBe(true);
    expect((section('auth').root as Record<string, unknown>).password).toBe('[redacted]');
    expect(section('hub').service_api_key).toBe('[redacted]');

    expect(yaml).toContain(`port: ${server.port}`);
    for (const text of [JSON.stringify(config), yaml]) {
      expect(text).not.toContain(ROOT_PASSWORD);
      expect(text).not.toContain(HUB_SERVICE_KEY);
      expect(text).not.toContain(server.adminKey);
      expect(text).not.toContain(server.readOnlyKey);
    }
  });

  it('denies a read-only key with the server permission error and no configuration', async () => {
    const response = await reader.getServerConfig();
    expect(response.success).toBe(false);
    expect(response.data).toBeUndefined();
    expect(response.error).toMatch(/NOPERM.*admin/);
  });

  it('refuses a caller without credentials', async () => {
    const response = await anonymous.getServerConfig();
    expect(response.success).toBe(false);
    expect(response.data).toBeUndefined();
    expect(response.error).toMatch(/401/);
  });
});

describe('key.type and memory.usage (structure inspector)', () => {
  beforeAll(async () => {
    expect((await admin.set('contract:string', 'hello')).success).toBe(true);
    expect((await admin.hashSet('contract:hash', 'f1', 'v1')).success).toBe(true);
    expect((await admin.listPush('contract:list', 'right', 'a')).success).toBe(true);
    expect((await admin.setAdd('contract:set', 'm')).success).toBe(true);
    expect((await admin.zsetAdd('contract:zset', 'm', 1.5)).success).toBe(true);
  });

  it.each([
    ['contract:string', 'string'],
    ['contract:hash', 'hash'],
    ['contract:list', 'list'],
    ['contract:set', 'set'],
    ['contract:zset', 'zset'],
  ])('reports %s as type %s', async (key, type) => {
    const response = await admin.keyType(key);
    expect(response).toEqual({ success: true, data: type });
  });

  it.each(['contract:string', 'contract:hash', 'contract:list', 'contract:set', 'contract:zset'])(
    'reports the memory used by %s',
    async (key) => {
      const response = await admin.memoryUsage(key);
      expect(response.success, response.error).toBe(true);
      expect(response.data!.bytes).toBeGreaterThan(0);
      expect(response.data!.human).toMatch(/^\d+(\.\d+)?[KMG]?B$/);
    },
  );
});

describe('missing keys', () => {
  it('reports type none for a key that does not exist', async () => {
    expect(await admin.keyType('contract:ghost')).toEqual({ success: true, data: 'none' });
  });

  it('reports memory.usage of a missing key as an error naming the key', async () => {
    const response = await admin.memoryUsage('contract:ghost');
    expect(response.success).toBe(false);
    expect(response.data).toBeUndefined();
    expect(response.error).toMatch(/contract:ghost.*does not exist/);
  });

  // The inspector reads contents only after key.type confirmed the structure,
  // but the client must still pass the server's answer through unchanged.
  it('reads a missing hash and sorted set as empty', async () => {
    expect(await admin.hashGetAll('contract:ghost')).toEqual({ success: true, data: {} });
    expect(await admin.zsetRange('contract:ghost')).toEqual({ success: true, data: [] });
  });

  it('surfaces the server error for a missing list and set', async () => {
    expect(await admin.listRange('contract:ghost')).toEqual({ success: false, error: 'Not found' });
    expect(await admin.setMembers('contract:ghost')).toEqual({ success: false, error: 'Not found' });
  });
});

describe('permission errors', () => {
  it('lets a read-only key inspect but not modify', async () => {
    expect(await reader.keyType('contract:hash')).toEqual({ success: true, data: 'hash' });
    expect((await reader.memoryUsage('contract:hash')).success).toBe(true);
    expect(await reader.hashGetAll('contract:hash')).toEqual({ success: true, data: { f1: 'v1' } });

    const write = await reader.hashSet('contract:hash', 'f2', 'v2');
    expect(write.success).toBe(false);
    expect(write.error).toMatch(/NOPERM.*write.*hash:contract:hash/);
    expect(await admin.hashGetAll('contract:hash')).toEqual({ success: true, data: { f1: 'v1' } });
  });

  it('reports an unknown API key as an authentication error', async () => {
    const response = await server.client('sk_not_a_real_key').keyType('contract:hash');
    expect(response.success).toBe(false);
    expect(response.error).toMatch(/401/);
  });

  it('reports a missing API key as an authentication error', async () => {
    const response = await anonymous.memoryUsage('contract:hash');
    expect(response.success).toBe(false);
    expect(response.error).toMatch(/401/);
  });
});

describe('encoded key names', () => {
  const names = [
    'path/with/slashes',
    'spaces and ?query=1&x#fragment',
    'percent%2Fliteral%20',
    'unicode-ключ-🔑',
    'colon:separated:{braces}',
  ];

  it.each(names)('round-trips string key %j through REST and the envelope', async (key) => {
    expect((await admin.set(key, 'value')).success).toBe(true);
    expect(await admin.get(key)).toEqual({ success: true, data: 'value' });
    expect(await admin.keyType(key)).toEqual({ success: true, data: 'string' });
    expect((await admin.memoryUsage(key)).success).toBe(true);
    expect((await admin.delete(key)).success).toBe(true);
    expect(await admin.keyType(key)).toEqual({ success: true, data: 'none' });
  });

  it.each(names)('inspects a hash named %j', async (raw) => {
    const key = `hash ${raw}`;
    expect((await admin.hashSet(key, 'field/1', 'v')).success).toBe(true);
    expect(await admin.keyType(key)).toEqual({ success: true, data: 'hash' });
    expect(await admin.hashGetAll(key)).toEqual({ success: true, data: { 'field/1': 'v' } });
    expect((await admin.hashDelete(key, 'field/1')).success).toBe(true);
    expect(await admin.keyType(key)).toEqual({ success: true, data: 'none' });
  });
});
