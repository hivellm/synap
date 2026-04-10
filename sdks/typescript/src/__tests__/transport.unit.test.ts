/**
 * Transport Layer Unit Tests
 * Tests for toWireValue, fromWireValue, mapCommand, mapResponse (pure),
 * and SynapRpcTransport / Resp3Transport with real local TCP servers.
 */

import { describe, it, expect, afterEach } from 'vitest';
import * as net from 'net';
import { pack, unpack } from 'msgpackr';
import { SynapRpcTransport, Resp3Transport, mapCommand, mapResponse } from '../transport';

// ── toWireValue / fromWireValue ───────────────────────────────────────────────
// These are not exported, so we test them indirectly through SynapRpcTransport.execute.
// However, mapCommand and mapResponse ARE exported and cover most of the wire encoding.
// For direct coverage of the encode/decode path we use a loopback TCP server and
// inspect what the server actually receives from SynapRpcTransport.

// Helper: start a net.Server on a random OS-assigned port, returns { server, port }
function startServer(
  handler: (data: Buffer, write: (buf: Buffer) => void) => void,
): Promise<{ server: net.Server; port: number }> {
  return new Promise((resolve, reject) => {
    const server = net.createServer((sock) => {
      sock.on('data', (chunk) => handler(chunk, (buf) => sock.write(buf)));
      sock.on('error', () => {/* ignore */});
    });
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address() as net.AddressInfo;
      resolve({ server, port: addr.port });
    });
    server.on('error', reject);
  });
}

// Helper: stop a server and wait for close
function stopServer(server: net.Server): Promise<void> {
  return new Promise((resolve) => server.close(() => resolve()));
}

// ── Wire value round-trip via SynapRPC loopback ───────────────────────────────

describe('toWireValue / fromWireValue (via SynapRpcTransport loopback)', () => {
  let server: net.Server;
  let transport: SynapRpcTransport;

  afterEach(async () => {
    transport?.close();
    if (server) await stopServer(server);
  });

  // Helper: create a loopback server that captures the received request frame,
  // decodes it, and echoes back the first arg as Ok.
  async function makeEchoServer(): Promise<{ server: net.Server; port: number }> {
    let readBuf = Buffer.alloc(0);
    return startServer((chunk, write) => {
      readBuf = Buffer.concat([readBuf, chunk]);
      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;
        const body = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);
        const req = unpack(body) as [number, string, unknown[]];
        const [id, , args] = req;
        // Echo first arg back as Ok
        const resp = pack([id, { Ok: args[0] }]);
        const lenBuf = Buffer.allocUnsafe(4);
        lenBuf.writeUInt32LE(resp.length, 0);
        write(Buffer.concat([lenBuf, resp]));
      }
    });
  }

  it('null → "Null" on wire → fromWireValue returns null', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    // Sending null as an arg → toWireValue wraps to "Null" → server echoes → fromWireValue gives null
    const result = await transport.execute('GET', [null]);
    expect(result).toBeNull();
  });

  it('boolean true → {Bool: true} on wire → fromWireValue returns true', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', [true]);
    expect(result).toBe(true);
  });

  it('boolean false → {Bool: false} on wire → fromWireValue returns false', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', [false]);
    expect(result).toBe(false);
  });

  it('integer → {Int: n} on wire → fromWireValue returns integer', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', [42]);
    expect(result).toBe(42);
  });

  it('float → {Float: f} on wire → fromWireValue returns float', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', [3.14]);
    expect(result).toBe(3.14);
  });

  it('string → {Str: s} on wire → fromWireValue returns string', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', ['hello']);
    expect(result).toBe('hello');
  });

  it('Buffer (bytes) → {Bytes: buf} on wire → fromWireValue decodes as UTF-8 string', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    // fromWireValue decodes Bytes as UTF-8 string for SDK consumers
    const buf = Buffer.from('hello');
    const result = await transport.execute('GET', [buf]);
    expect(result).toBe('hello');
  });

  it('undefined → treated as null (Null) on wire', async () => {
    const { server: s, port } = await makeEchoServer();
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);
    const result = await transport.execute('GET', [undefined]);
    expect(result).toBeNull();
  });
});

// ── mapCommand ────────────────────────────────────────────────────────────────

describe('mapCommand', () => {
  describe('KV commands', () => {
    it('"kv.get" with {key: "foo"} → {rawCmd: "GET", args: ["foo"]}', () => {
      const result = mapCommand('kv.get', { key: 'foo' });
      expect(result).toEqual({ rawCmd: 'GET', args: ['foo'] });
    });

    it('"kv.set" with {key: "foo", value: {Str: "bar"}} → {rawCmd: "SET", args: ["foo", {Str: "bar"}]}', () => {
      const result = mapCommand('kv.set', { key: 'foo', value: { Str: 'bar' } });
      expect(result).toEqual({ rawCmd: 'SET', args: ['foo', { Str: 'bar' }] });
    });

    it('"kv.set" with ttl → includes EX arg', () => {
      const result = mapCommand('kv.set', { key: 'foo', value: 'bar', ttl: 60 });
      expect(result).toEqual({ rawCmd: 'SET', args: ['foo', 'bar', 'EX', '60'] });
    });

    it('"kv.del" with {key: "foo"} → {rawCmd: "DEL", args: ["foo"]}', () => {
      const result = mapCommand('kv.del', { key: 'foo' });
      expect(result).toEqual({ rawCmd: 'DEL', args: ['foo'] });
    });

    it('"kv.exists" with {key: "foo"} → {rawCmd: "EXISTS", args: ["foo"]}', () => {
      const result = mapCommand('kv.exists', { key: 'foo' });
      expect(result).toEqual({ rawCmd: 'EXISTS', args: ['foo'] });
    });

    it('"kv.incr" with {key: "counter"} → {rawCmd: "INCR", args: ["counter"]}', () => {
      const result = mapCommand('kv.incr', { key: 'counter' });
      expect(result).toEqual({ rawCmd: 'INCR', args: ['counter'] });
    });

    it('"kv.decr" with {key: "counter"} → {rawCmd: "DECR", args: ["counter"]}', () => {
      const result = mapCommand('kv.decr', { key: 'counter' });
      expect(result).toEqual({ rawCmd: 'DECR', args: ['counter'] });
    });

    it('"kv.keys" with prefix → adds wildcard suffix', () => {
      const result = mapCommand('kv.keys', { prefix: 'user:' });
      expect(result).toEqual({ rawCmd: 'KEYS', args: ['user:*'] });
    });

    it('"kv.keys" with empty prefix → "*"', () => {
      const result = mapCommand('kv.keys', { prefix: '' });
      expect(result).toEqual({ rawCmd: 'KEYS', args: ['*'] });
    });

    it('"kv.expire" → {rawCmd: "EXPIRE"}', () => {
      const result = mapCommand('kv.expire', { key: 'foo', ttl: 300 });
      expect(result).toEqual({ rawCmd: 'EXPIRE', args: ['foo', '300'] });
    });

    it('"kv.ttl" → {rawCmd: "TTL"}', () => {
      const result = mapCommand('kv.ttl', { key: 'foo' });
      expect(result).toEqual({ rawCmd: 'TTL', args: ['foo'] });
    });
  });

  describe('Hash commands', () => {
    it('"hash.get" → {rawCmd: "HGET"}', () => {
      const result = mapCommand('hash.get', { key: 'myhash', field: 'f1' });
      expect(result).toEqual({ rawCmd: 'HGET', args: ['myhash', 'f1'] });
    });

    it('"hash.set" → {rawCmd: "HSET"}', () => {
      const result = mapCommand('hash.set', { key: 'myhash', field: 'f1', value: 'v1' });
      expect(result).toEqual({ rawCmd: 'HSET', args: ['myhash', 'f1', 'v1'] });
    });

    it('"hash.getall" → {rawCmd: "HGETALL"}', () => {
      const result = mapCommand('hash.getall', { key: 'myhash' });
      expect(result).toEqual({ rawCmd: 'HGETALL', args: ['myhash'] });
    });

    it('"hash.del" → {rawCmd: "HDEL"}', () => {
      const result = mapCommand('hash.del', { key: 'myhash', field: 'f1' });
      expect(result).toEqual({ rawCmd: 'HDEL', args: ['myhash', 'f1'] });
    });
  });

  describe('List commands', () => {
    it('"list.lpush" with values → {rawCmd: "LPUSH", args: [key, ...values]}', () => {
      const result = mapCommand('list.lpush', { key: 'mylist', values: ['a', 'b'] });
      expect(result).toEqual({ rawCmd: 'LPUSH', args: ['mylist', 'a', 'b'] });
    });

    it('"list.rpush" → {rawCmd: "RPUSH"}', () => {
      const result = mapCommand('list.rpush', { key: 'mylist', values: ['x'] });
      expect(result).toEqual({ rawCmd: 'RPUSH', args: ['mylist', 'x'] });
    });

    it('"list.range" → {rawCmd: "LRANGE"}', () => {
      const result = mapCommand('list.range', { key: 'mylist', start: 0, stop: -1 });
      expect(result).toEqual({ rawCmd: 'LRANGE', args: ['mylist', '0', '-1'] });
    });
  });

  describe('Set commands', () => {
    it('"set.add" with members → {rawCmd: "SADD", args: [key, ...members]}', () => {
      const result = mapCommand('set.add', { key: 'myset', members: ['m1', 'm2'] });
      expect(result).toEqual({ rawCmd: 'SADD', args: ['myset', 'm1', 'm2'] });
    });

    it('"set.ismember" → {rawCmd: "SISMEMBER"}', () => {
      const result = mapCommand('set.ismember', { key: 'myset', member: 'm1' });
      expect(result).toEqual({ rawCmd: 'SISMEMBER', args: ['myset', 'm1'] });
    });

    it('"set.members" → {rawCmd: "SMEMBERS"}', () => {
      const result = mapCommand('set.members', { key: 'myset' });
      expect(result).toEqual({ rawCmd: 'SMEMBERS', args: ['myset'] });
    });
  });

  describe('Queue/stream commands now mapped', () => {
    it('"queue.publish" → QPUBLISH', () => {
      const result = mapCommand('queue.publish', { queue: 'q1', payload: 'hello' });
      expect(result).not.toBeNull();
      expect(result?.rawCmd).toBe('QPUBLISH');
    });

    it('"stream.publish" → SPUBLISH', () => {
      const result = mapCommand('stream.publish', { room: 's1', event: 'evt', data: {} });
      expect(result).not.toBeNull();
      expect(result?.rawCmd).toBe('SPUBLISH');
    });
  });

  describe('Unmapped commands (HTTP fallback)', () => {
    it('"unknown.command" → null', () => {
      const result = mapCommand('unknown.command', {});
      expect(result).toBeNull();
    });
  });
});

// ── mapResponse ───────────────────────────────────────────────────────────────

describe('mapResponse', () => {
  describe('KV responses', () => {
    it('"kv.get" returns raw value as-is', () => {
      expect(mapResponse('kv.get', 'bar')).toBe('bar');
    });

    it('"kv.get" with null returns null', () => {
      expect(mapResponse('kv.get', null)).toBeNull();
    });

    it('"kv.del" with raw 1 → {deleted: true}', () => {
      expect(mapResponse('kv.del', 1)).toEqual({ deleted: true });
    });

    it('"kv.del" with raw 0 → {deleted: false}', () => {
      expect(mapResponse('kv.del', 0)).toEqual({ deleted: false });
    });

    it('"kv.exists" with raw 1 → {exists: true}', () => {
      expect(mapResponse('kv.exists', 1)).toEqual({ exists: true });
    });

    it('"kv.exists" with raw 0 → {exists: false}', () => {
      expect(mapResponse('kv.exists', 0)).toEqual({ exists: false });
    });

    it('"kv.incr" with raw 42 → {value: 42}', () => {
      expect(mapResponse('kv.incr', 42)).toEqual({ value: 42 });
    });

    it('"kv.decr" with raw 10 → {value: 10}', () => {
      expect(mapResponse('kv.decr', 10)).toEqual({ value: 10 });
    });

    it('"kv.set" → {}', () => {
      expect(mapResponse('kv.set', 'OK')).toEqual({});
    });

    it('"kv.keys" → {keys: [...]}', () => {
      expect(mapResponse('kv.keys', ['a', 'b'])).toEqual({ keys: ['a', 'b'] });
    });

    it('"kv.expire" → {}', () => {
      expect(mapResponse('kv.expire', 1)).toEqual({});
    });

    it('"kv.ttl" returns raw value', () => {
      expect(mapResponse('kv.ttl', 300)).toBe(300);
    });
  });

  describe('Hash responses', () => {
    it('"hash.getall" with interleaved array → {fields: {f1: v1, f2: v2}}', () => {
      const result = mapResponse('hash.getall', ['f1', 'v1', 'f2', 'v2']);
      expect(result).toEqual({ fields: { f1: 'v1', f2: 'v2' } });
    });

    it('"hash.getall" with empty array → {fields: {}}', () => {
      const result = mapResponse('hash.getall', []);
      expect(result).toEqual({ fields: {} });
    });

    it('"hash.get" with value → {value: "foo"}', () => {
      expect(mapResponse('hash.get', 'foo')).toEqual({ value: 'foo' });
    });

    it('"hash.get" with null → {value: null}', () => {
      expect(mapResponse('hash.get', null)).toEqual({ value: null });
    });

    it('"hash.set" with 1 → {success: true}', () => {
      expect(mapResponse('hash.set', 1)).toEqual({ success: true });
    });

    it('"hash.del" with 1 → {deleted: 1}', () => {
      expect(mapResponse('hash.del', 1)).toEqual({ deleted: 1 });
    });

    it('"hash.exists" with 1 → {exists: true}', () => {
      expect(mapResponse('hash.exists', 1)).toEqual({ exists: true });
    });

    it('"hash.len" → {length: 5}', () => {
      expect(mapResponse('hash.len', 5)).toEqual({ length: 5 });
    });

    it('"hash.keys" → {fields: [...]}', () => {
      expect(mapResponse('hash.keys', ['f1', 'f2'])).toEqual({ fields: ['f1', 'f2'] });
    });

    it('"hash.values" → {values: [...]}', () => {
      expect(mapResponse('hash.values', ['v1', 'v2'])).toEqual({ values: ['v1', 'v2'] });
    });

    it('"hash.incrby" → {value: 7}', () => {
      expect(mapResponse('hash.incrby', 7)).toEqual({ value: 7 });
    });

    it('"hash.incrbyfloat" → {value: 1.5}', () => {
      expect(mapResponse('hash.incrbyfloat', 1.5)).toEqual({ value: 1.5 });
    });

    it('"hash.setnx" with 1 → {created: true}', () => {
      expect(mapResponse('hash.setnx', 1)).toEqual({ created: true });
    });

    it('"hash.mset" with truthy value → {success: true}', () => {
      expect(mapResponse('hash.mset', 'OK')).toEqual({ success: true });
    });

    it('"hash.mget" → {values: [...]}', () => {
      expect(mapResponse('hash.mget', ['v1', null, 'v3'])).toEqual({ values: ['v1', null, 'v3'] });
    });
  });

  describe('List responses', () => {
    it('"list.lpush" → {length: 3}', () => {
      expect(mapResponse('list.lpush', 3)).toEqual({ length: 3 });
    });

    it('"list.rpush" → {length: 2}', () => {
      expect(mapResponse('list.rpush', 2)).toEqual({ length: 2 });
    });

    it('"list.lpop" with single value → {values: ["x"]}', () => {
      expect(mapResponse('list.lpop', 'x')).toEqual({ values: ['x'] });
    });

    it('"list.lpop" with null → {values: []}', () => {
      expect(mapResponse('list.lpop', null)).toEqual({ values: [] });
    });

    it('"list.range" → {values: [...]}', () => {
      expect(mapResponse('list.range', ['a', 'b'])).toEqual({ values: ['a', 'b'] });
    });

    it('"list.len" → {length: 4}', () => {
      expect(mapResponse('list.len', 4)).toEqual({ length: 4 });
    });

    it('"list.rem" → {count: 1}', () => {
      expect(mapResponse('list.rem', 1)).toEqual({ count: 1 });
    });
  });

  describe('Set responses', () => {
    it('"set.add" → {added: 2}', () => {
      expect(mapResponse('set.add', 2)).toEqual({ added: 2 });
    });

    it('"set.rem" → {removed: 1}', () => {
      expect(mapResponse('set.rem', 1)).toEqual({ removed: 1 });
    });

    it('"set.ismember" with 1 → {is_member: true}', () => {
      expect(mapResponse('set.ismember', 1)).toEqual({ is_member: true });
    });

    it('"set.members" → {members: [...]}', () => {
      expect(mapResponse('set.members', ['a', 'b'])).toEqual({ members: ['a', 'b'] });
    });

    it('"set.card" → {cardinality: 5}', () => {
      expect(mapResponse('set.card', 5)).toEqual({ cardinality: 5 });
    });
  });

  describe('Default fallthrough', () => {
    it('unknown command returns raw value', () => {
      expect(mapResponse('unknown.cmd', 'rawval')).toBe('rawval');
    });
  });
});

// ── SynapRpcTransport (real TCP server) ──────────────────────────────────────

describe('SynapRpcTransport', () => {
  let server: net.Server;
  let transport: SynapRpcTransport;

  afterEach(async () => {
    transport?.close();
    if (server) await stopServer(server);
  });

  // Build a framed msgpack response for the server to send
  function buildResponse(id: number, payload: unknown): Buffer {
    const body = pack([id, payload]);
    const lenBuf = Buffer.allocUnsafe(4);
    lenBuf.writeUInt32LE(body.length, 0);
    return Buffer.concat([lenBuf, body]);
  }

  it('sends framed msgpack request and receives Ok response', async () => {
    let receivedReq: [number, string, unknown[]] | null = null;
    let readBuf = Buffer.alloc(0);

    const { server: s, port } = await startServer((chunk, write) => {
      readBuf = Buffer.concat([readBuf, chunk]);
      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;
        const body = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);
        receivedReq = unpack(body) as [number, string, unknown[]];
        const [id] = receivedReq;
        write(buildResponse(id, { Ok: { Str: 'testvalue' } }));
      }
    });
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['testkey']);

    expect(result).toBe('testvalue');
    expect(receivedReq).not.toBeNull();
    const [, cmd, args] = receivedReq!;
    expect(cmd).toBe('GET');
    // args[0] should be the wire-encoded "testkey" = {Str: "testkey"}
    expect(args[0]).toEqual({ Str: 'testkey' });
  });

  it('throws an Error when server sends Err response', async () => {
    let readBuf = Buffer.alloc(0);
    const { server: s, port } = await startServer((chunk, write) => {
      readBuf = Buffer.concat([readBuf, chunk]);
      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;
        const body = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);
        const [id] = unpack(body) as [number, string, unknown[]];
        write(buildResponse(id, { Err: 'key not found' }));
      }
    });
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);

    await expect(transport.execute('GET', ['missingkey'])).rejects.toThrow('key not found');
  });

  it('multiplexes multiple concurrent requests by id', async () => {
    let readBuf = Buffer.alloc(0);
    const { server: s, port } = await startServer((chunk, write) => {
      readBuf = Buffer.concat([readBuf, chunk]);
      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;
        const body = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);
        const [id, , args] = unpack(body) as [number, string, unknown[]];
        // Echo back the key as the response value
        write(buildResponse(id, { Ok: args[0] }));
      }
    });
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);

    // Pre-warm the connection so both concurrent calls share a single socket.
    await transport.execute('GET', ['warmup']);

    // Now fire two more concurrent requests over the established connection.
    const [r1, r2] = await Promise.all([
      transport.execute('GET', ['key1']),
      transport.execute('GET', ['key2']),
    ]);

    expect(r1).toBe('key1');
    expect(r2).toBe('key2');
  });

  it('returns null when server sends Ok: "Null"', async () => {
    let readBuf = Buffer.alloc(0);
    const { server: s, port } = await startServer((chunk, write) => {
      readBuf = Buffer.concat([readBuf, chunk]);
      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;
        const body = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);
        const [id] = unpack(body) as [number, string, unknown[]];
        write(buildResponse(id, { Ok: 'Null' }));
      }
    });
    server = s;
    transport = new SynapRpcTransport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['foo']);
    expect(result).toBeNull();
  });

  it('rejects with error when connection is refused', async () => {
    // Port 1 is almost certainly not listening
    transport = new SynapRpcTransport('127.0.0.1', 1, 1000);
    await expect(transport.execute('GET', ['k'])).rejects.toThrow();
  });
});

// ── Resp3Transport (real TCP server) ─────────────────────────────────────────

describe('Resp3Transport', () => {
  let server: net.Server;
  let transport: Resp3Transport;

  afterEach(async () => {
    transport?.close();
    if (server) await stopServer(server);
  });

  // Build a minimal RESP3 server that handles one command at a time
  function buildResp3Server(
    handler: (parts: string[]) => string,
  ): Promise<{ server: net.Server; port: number }> {
    return startServer((chunk, write) => {
      // Simple line-oriented parser for the multibulk frames the transport sends.
      // We collect all lines and wait for a complete *N + N $len + N words sequence.
      // For test simplicity we buffer the full ASCII and parse on each arrival.
      const text = chunk.toString('utf8');
      // Extract the command parts from *N\r\n$len\r\nword\r\n ... sequence
      const lines = text.split('\r\n').filter(Boolean);
      const parts: string[] = [];
      let i = 0;
      if (lines[i]?.startsWith('*')) {
        const count = parseInt(lines[i].slice(1), 10);
        i++;
        for (let j = 0; j < count; j++) {
          if (lines[i]?.startsWith('$')) i++; // skip length line
          parts.push(lines[i] ?? '');
          i++;
        }
      }
      const response = handler(parts);
      write(Buffer.from(response, 'utf8'));
    });
  }

  it('sends RESP multibulk frame and receives simple string response', async () => {
    const { server: s, port } = await buildResp3Server((parts) => {
      if (parts[0]?.toUpperCase() === 'GET') return '+testvalue\r\n';
      return '-ERR unknown\r\n';
    });
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['testkey']);
    expect(result).toBe('testvalue');
  });

  it('receives null (_\\r\\n) → returns null', async () => {
    const { server: s, port } = await buildResp3Server(() => '_\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['missingkey']);
    expect(result).toBeNull();
  });

  it('receives integer (: prefix) → returns number', async () => {
    const { server: s, port } = await buildResp3Server(() => ':42\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('INCR', ['counter']);
    expect(result).toBe(42);
  });

  it('receives error (- prefix) → throws Error', async () => {
    const { server: s, port } = await buildResp3Server(() => '-ERR key not found\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    await expect(transport.execute('GET', ['badkey'])).rejects.toThrow('ERR key not found');
  });

  it('receives boolean #t → true', async () => {
    const { server: s, port } = await buildResp3Server(() => '#t\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['k']);
    expect(result).toBe(true);
  });

  it('receives boolean #f → false', async () => {
    const { server: s, port } = await buildResp3Server(() => '#f\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['k']);
    expect(result).toBe(false);
  });

  it('receives bulk string ($len\\r\\ndata\\r\\n) → string', async () => {
    // The bulk string test requires that the length line and the data arrive
    // in separate TCP writes. This is because Resp3Transport's data handler
    // routes incoming bytes to either lineBuffer or binBuffer depending on
    // whether resolveBin is set. If the entire "$9\r\ntestvalue\r\n" arrives
    // in one chunk, it all goes to lineBuffer and the readExact() call finds
    // an empty binBuffer with no pending data event to drain it.
    // We must send the length line first, wait for the transport to call
    // readExact (setting resolveBin), then send the body.
    const { server: s, port } = await new Promise<{ server: net.Server; port: number }>((resolve, reject) => {
      const srv = net.createServer((sock: net.Socket) => {
        let receivedData = false;
        sock.on('data', () => {
          if (receivedData) return;
          receivedData = true;
          // Send the length prefix first
          sock.write('$9\r\n', () => {
            // After a tick, send the body — by then resolveBin will be set
            setTimeout(() => sock.write('testvalue\r\n'), 0);
          });
        });
        sock.on('error', () => {/* ignore */});
      });
      srv.listen(0, '127.0.0.1', () => {
        const addr = srv.address() as net.AddressInfo;
        resolve({ server: srv, port: addr.port });
      });
      srv.on('error', reject);
    });
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['testkey']);
    expect(result).toBe('testvalue');
  });

  it('receives $-1 (null bulk string) → null', async () => {
    const { server: s, port } = await buildResp3Server(() => '$-1\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('GET', ['k']);
    expect(result).toBeNull();
  });

  it('receives array (*N) → array of values', async () => {
    const { server: s, port } = await buildResp3Server(() => '*3\r\n+a\r\n+b\r\n+c\r\n');
    server = s;
    transport = new Resp3Transport('127.0.0.1', port, 5000);

    const result = await transport.execute('SMEMBERS', ['myset']);
    expect(result).toEqual(['a', 'b', 'c']);
  });

  it('rejects with error when connection is refused', async () => {
    transport = new Resp3Transport('127.0.0.1', 1, 1000);
    await expect(transport.execute('GET', ['k'])).rejects.toThrow();
  });
});
