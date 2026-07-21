/**
 * Transaction parity S2S tests — MULTI → queued write → EXEC/DISCARD on every
 * transport (ADR 005: queued writes travel as TXQUEUE on native transports).
 * Requires a running Synap server.
 */

import { describe, it, expect, afterAll } from 'vitest';
import { Synap } from '../index';
import { SynapClient } from '../client';
import { UnsupportedCommandError } from '../types';

const HTTP_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const RPC_URL = process.env.SYNAP_RPC_URL || 'synap://localhost:15501';
const RESP3_URL = process.env.SYNAP_RESP3_URL || 'resp3://localhost:6379';

const uid = () => Math.random().toString(36).slice(2, 10);

const TRANSPORTS: Array<[string, string]> = [
  ['http', HTTP_URL],
  ['synaprpc', RPC_URL],
  ['resp3', RESP3_URL],
];

describe.each(TRANSPORTS)('Transactions (S2S, %s)', (_label, url) => {
  const synap = new Synap({ url, timeout: 10000 });

  afterAll(() => {
    synap.close();
  });

  it('queued write is invisible before EXEC and applied after', async () => {
    const clientId = `ts-tx-${uid()}`;
    const key = `tx:s2s:${uid()}`;

    await synap.transaction.multi({ clientId });
    await synap.kv.set(key, 'committed', { clientId });

    expect(await synap.kv.get(key)).toBeNull();

    const result = await synap.transaction.exec({ clientId });
    expect(result.success).toBe(true);
    expect(await synap.kv.get(key)).toBe('committed');

    await synap.kv.del(key);
  });

  it('DISCARD drops the queued write', async () => {
    const clientId = `ts-tx-discard-${uid()}`;
    const key = `tx:s2s:discard:${uid()}`;

    await synap.transaction.multi({ clientId });
    await synap.kv.set(key, 'dropped', { clientId });
    await synap.transaction.discard({ clientId });

    expect(await synap.kv.get(key)).toBeNull();
  });
});

describe('Transactions (S2S, native refusal)', () => {
  it('unqueueable command with clientId is refused on native transports', async () => {
    const client = new SynapClient({ url: RPC_URL, timeout: 10000 });
    const clientId = `ts-tx-refuse-${uid()}`;

    await client.sendCommand('transaction.multi', { client_id: clientId });
    await expect(
      client.sendCommand('sortedset.zadd', {
        key: `tx:z:${uid()}`,
        member: 'm',
        score: 1,
        client_id: clientId,
      }),
    ).rejects.toThrow(UnsupportedCommandError);
    await client.sendCommand('transaction.discard', { client_id: clientId });
    client.close();
  });
});
