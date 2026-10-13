// @vitest-environment happy-dom
/**
 * Smoke test of the structure inspector: each structure is created through a
 * real synap-server, then StructuresView is mounted against that server and
 * driven like a user would — pick the tab, type the key, press Inspect.
 */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import StructuresView from '@/views/StructuresView.vue';
import { useServersStore } from '@/stores/servers';
import { startTestServer, type TestServer } from '../support/server';

let server: TestServer;

beforeAll(async () => {
  server = await startTestServer();
  const admin = server.client(server.adminKey);
  // One at a time: the list's order is the order the pushes arrive in.
  const writes = [
    () => admin.hashSet('smoke:hash', 'name', 'synap'),
    () => admin.hashSet('smoke:hash', 'kind', 'store'),
    () => admin.listPush('smoke:list', 'right', 'first'),
    () => admin.listPush('smoke:list', 'right', 'second'),
    () => admin.setAdd('smoke:set', 'red'),
    () => admin.setAdd('smoke:set', 'blue'),
    () => admin.zsetAdd('smoke:zset', 'bronze', 1),
    () => admin.zsetAdd('smoke:zset', 'gold', 3),
  ];
  for (const write of writes) {
    const result = await write();
    expect(result.success, result.error).toBe(true);
  }
});

afterAll(async () => {
  await server?.stop();
});

/** StructuresView connected, through the servers store, to the test server. */
async function mountInspector(): Promise<VueWrapper> {
  const pinia = createPinia();
  setActivePinia(pinia);
  const store = useServersStore();
  const id = store.addServer({
    name: 'smoke',
    url: server.url,
    port: server.port,
    apiKey: server.adminKey,
  });
  expect(await store.connectServer(id)).toBe(true);
  store.activeServerId = id;
  return mount(StructuresView, { global: { plugins: [pinia] } });
}

async function inspect(wrapper: VueWrapper, tabLabel: string, key: string): Promise<void> {
  const tab = wrapper.findAll('button').find((b) => b.text() === tabLabel);
  expect(tab, `tab ${tabLabel}`).toBeDefined();
  await tab!.trigger('click');
  await wrapper.find('input[type="text"]').setValue(key);
  const button = wrapper.findAll('button').find((b) => b.text() === 'Inspect');
  await button!.trigger('click');
}

async function shows(wrapper: VueWrapper, ...texts: string[]): Promise<void> {
  await vi.waitFor(
    () => {
      const text = wrapper.text();
      for (const expected of texts) expect(text).toContain(expected);
    },
    { timeout: 10_000, interval: 50 },
  );
}

describe('structure inspector against a real server', () => {
  it('shows a hash: its type, both fields and its memory usage', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'Hash', 'smoke:hash');
    await shows(wrapper, 'type hash', '2 fields', 'name', 'synap', 'kind', 'store');
    expect(wrapper.text()).toMatch(/memory \d+(\.\d+)?[KMG]?B/);
    wrapper.unmount();
  });

  it('shows a list in order', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'List', 'smoke:list');
    await shows(wrapper, 'type list', '2 elements');
    expect(wrapper.text().indexOf('first')).toBeLessThan(wrapper.text().indexOf('second'));
    wrapper.unmount();
  });

  it('shows a set', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'Set', 'smoke:set');
    await shows(wrapper, 'type set', '2 members', 'red', 'blue');
    wrapper.unmount();
  });

  it('shows a sorted set ranked by score', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'Sorted Set', 'smoke:zset');
    await shows(wrapper, 'type zset', '2 members', 'bronze', 'gold');
    expect(wrapper.text().indexOf('bronze')).toBeLessThan(wrapper.text().indexOf('gold'));
    wrapper.unmount();
  });

  it('reports a key that does not exist', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'Hash', 'ghost');
    await shows(wrapper, 'Key "ghost" does not exist');
    wrapper.unmount();
  });

  it('reports a key of another type instead of clearing itself', async () => {
    const wrapper = await mountInspector();
    await inspect(wrapper, 'List', 'smoke:hash');
    await shows(wrapper, 'type hash', 'Key "smoke:hash" is a hash, not a list');
    wrapper.unmount();
  });
});
