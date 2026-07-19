/**
 * KV Watch Unit Tests
 *
 * Drives `kv.watch()` against a mocked SynapRPC transport: envelope decoding,
 * mode passthrough, teardown → KV.UNWATCH, and the `withValueFetch` operator.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { firstValueFrom } from 'rxjs';
import { take, toArray } from 'rxjs/operators';
import { KVStore, withValueFetch } from '../kv';
import type { WatchEvent } from '../types';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

type PushCallback = (envelope: Record<string, unknown>) => void;

/** A mock transport that records watchPush calls and exposes the callback. */
function mockRpcTransport() {
  const cancel = vi.fn();
  const calls: Array<{ pattern: string; mode: string }> = [];
  let onEvent: PushCallback = () => undefined;

  const rpc = {
    watchPush: vi.fn(async (pattern: string, mode: string, cb: PushCallback) => {
      calls.push({ pattern, mode });
      onEvent = cb;
      return { subscriberId: 'sub-1', cancel };
    }),
  };

  return { rpc, cancel, calls, push: (envelope: Record<string, unknown>) => onEvent(envelope) };
}

function kvWithRpc(rpc: unknown): { kv: KVStore; client: SynapClient } {
  const client = createMockClient();
  (client as any).synapRpcTransport = () => rpc;
  return { kv: new KVStore(client), client };
}

/** Let the async setup inside watch() run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('kv.watch()', () => {
  let transport: ReturnType<typeof mockRpcTransport>;
  let kv: KVStore;

  beforeEach(() => {
    transport = mockRpcTransport();
    ({ kv } = kvWithRpc(transport.rpc));
  });

  it('decodes envelopes into typed events', async () => {
    const events: WatchEvent<string>[] = [];
    const sub = kv.watch<string>('user:1').subscribe((e) => events.push(e));
    await settle();

    transport.push({ key: 'user:1', event: 'set', version: 1, value: 'alice' });
    transport.push({ key: 'user:1', event: 'del', version: 2 });

    expect(events).toEqual([
      { key: 'user:1', event: 'set', version: 1, value: 'alice', truncated: false },
      { key: 'user:1', event: 'del', version: 2, value: undefined, truncated: false },
    ]);
    sub.unsubscribe();
  });

  it('parses JSON-encoded values back to structured form', async () => {
    const events: WatchEvent<{ name: string }>[] = [];
    const sub = kv.watch<{ name: string }>('user:1').subscribe((e) => events.push(e));
    await settle();

    transport.push({ key: 'user:1', event: 'set', version: 1, value: '{"name":"alice"}' });

    expect(events[0]?.value).toEqual({ name: 'alice' });
    sub.unsubscribe();
  });

  it('passes the pattern and default mode to the transport', async () => {
    const sub = kv.watch('user:*').subscribe();
    await settle();

    expect(transport.calls).toEqual([{ pattern: 'user:*', mode: 'value' }]);
    sub.unsubscribe();
  });

  it('passes notify mode through', async () => {
    const sub = kv.watch('user:*', { mode: 'notify' }).subscribe();
    await settle();

    expect(transport.calls).toEqual([{ pattern: 'user:*', mode: 'notify' }]);
    sub.unsubscribe();
  });

  it('tears the push connection down on unsubscribe', async () => {
    const sub = kv.watch('user:1').subscribe();
    await settle();
    expect(transport.cancel).not.toHaveBeenCalled();

    sub.unsubscribe();

    expect(transport.cancel).toHaveBeenCalledTimes(1);
  });

  it('cancels a subscription that was torn down mid-handshake', async () => {
    const sub = kv.watch('user:1').subscribe();
    sub.unsubscribe(); // before the async watchPush resolves
    await settle();

    expect(transport.cancel).toHaveBeenCalledTimes(1);
  });

  it('errors without the synap:// transport', async () => {
    const client = createMockClient();
    const httpKv = new KVStore(client);

    await expect(firstValueFrom(httpKv.watch('k'))).rejects.toThrow(/synap:\/\//);
  });
});

describe('withValueFetch', () => {
  it('re-GETs when the envelope arrived without a value', async () => {
    const transport = mockRpcTransport();
    const { kv, client } = kvWithRpc(transport.rpc);
    vi.mocked(client.sendCommand).mockResolvedValue('fetched-value');

    const collected = firstValueFrom(
      kv.watch<string>('big').pipe(withValueFetch<string>(kv), take(2), toArray()),
    );
    await settle();

    transport.push({ key: 'big', event: 'set', version: 1, truncated: true });
    transport.push({ key: 'big', event: 'set', version: 2, value: 'inline' });

    const events = await collected;
    expect(events[0]).toEqual({
      key: 'big',
      event: 'set',
      version: 1,
      value: 'fetched-value',
      truncated: false,
    });
    expect(events[1]?.value).toBe('inline');
    expect(client.sendCommand).toHaveBeenCalledWith('kv.get', { key: 'big' });
    expect(client.sendCommand).toHaveBeenCalledTimes(1);
  });

  it('does not fetch for terminal events', async () => {
    const transport = mockRpcTransport();
    const { kv, client } = kvWithRpc(transport.rpc);

    const collected = firstValueFrom(
      kv.watch<string>('k').pipe(withValueFetch<string>(kv), take(1)),
    );
    await settle();

    transport.push({ key: 'k', event: 'del', version: 3 });

    const event = await collected;
    expect(event.value).toBeUndefined();
    expect(client.sendCommand).not.toHaveBeenCalled();
  });
});
