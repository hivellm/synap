/**
 * Synap TypeScript SDK - Binary TCP Transports
 *
 * Implements SynapRPC (MessagePack over TCP) and RESP3 (Redis-compatible text
 * protocol over TCP) transports, alongside the original HTTP REST transport.
 *
 * The encoding of SynapRPC wire values mirrors Rust's rmp_serde externally-tagged
 * enum format:
 *   - Unit variants (Null)      → bare msgpack string  "Null"
 *   - Newtype variants (Str, …) → single-key msgpack map  {"Str": value}
 *   - Structs (Request, …)      → msgpack array  [field0, field1, …]
 */

import * as net from 'net';
import { pack, unpack } from 'msgpackr';

// ── Transport mode ────────────────────────────────────────────────────────────

export type TransportMode = 'synaprpc' | 'resp3' | 'http';

// ── Wire value helpers ────────────────────────────────────────────────────────

/**
 * Convert a JavaScript value into the externally-tagged WireValue envelope
 * that rmp_serde expects on the wire.
 */
function toWireValue(v: unknown): unknown {
  if (v === null || v === undefined) return 'Null';
  if (typeof v === 'string') return { Str: v };
  if (typeof v === 'boolean') return { Bool: v };
  if (typeof v === 'number') {
    return Number.isInteger(v) ? { Int: v } : { Float: v };
  }
  if (v instanceof Uint8Array || Buffer.isBuffer(v)) return { Bytes: v };
  return { Str: String(v) };
}

/**
 * Unwrap a WireValue envelope back to a plain JavaScript value.
 */
function fromWireValue(wire: unknown): unknown {
  if (wire === 'Null' || wire === null || wire === undefined) return null;
  if (typeof wire === 'object') {
    const w = wire as Record<string, unknown>;
    if ('Str' in w) return w.Str;
    if ('Int' in w) return w.Int;
    if ('Float' in w) return w.Float;
    if ('Bool' in w) return w.Bool;
    if ('Bytes' in w) {
      // Stored KV values are UTF-8 strings on the wire; decode for SDK consumers.
      const b = w.Bytes;
      if (b instanceof Uint8Array || Buffer.isBuffer(b)) {
        return Buffer.from(b as Uint8Array).toString('utf8');
      }
      if (Array.isArray(b)) {
        return Buffer.from(b as number[]).toString('utf8');
      }
      return b;
    }
    if ('Array' in w) {
      return (w.Array as unknown[]).map(fromWireValue);
    }
    if ('Map' in w) {
      const pairs = w.Map as [unknown, unknown][];
      const obj: Record<string, unknown> = {};
      for (const [k, val] of pairs) {
        obj[String(fromWireValue(k))] = fromWireValue(val);
      }
      return obj;
    }
  }
  return wire;
}

// ── SynapRPC transport ────────────────────────────────────────────────────────

interface Pending {
  resolve: (value: unknown) => void;
  reject: (err: Error) => void;
}

/**
 * Persistent TCP connection to the SynapRPC listener.
 * Requests are multiplexed by request ID; responses are matched and resolved.
 */
export class SynapRpcTransport {
  private readonly host: string;
  private readonly port: number;
  private readonly timeoutMs: number;
  private socket: net.Socket | null = null;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private readBuffer = Buffer.alloc(0);

  constructor(host: string, port: number, timeoutMs: number) {
    this.host = host;
    this.port = port;
    this.timeoutMs = timeoutMs;
  }

  private connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const sock = new net.Socket();
      sock.setTimeout(this.timeoutMs);

      sock.once('connect', () => {
        this.socket = sock;
        this.readBuffer = Buffer.alloc(0);
        resolve();
      });

      sock.on('data', (chunk: Buffer) => {
        this.readBuffer = Buffer.concat([this.readBuffer, chunk]);
        this.drainFrames();
      });

      sock.on('error', (err) => {
        this.socket = null;
        // Reject all pending requests on error.
        for (const { reject: rej } of this.pending.values()) {
          rej(err);
        }
        this.pending.clear();
        reject(err);
      });

      sock.on('close', () => {
        this.socket = null;
        for (const { reject: rej } of this.pending.values()) {
          rej(new Error('SynapRPC connection closed'));
        }
        this.pending.clear();
      });

      sock.on('timeout', () => {
        sock.destroy(new Error('SynapRPC connection timeout'));
      });

      sock.connect(this.port, this.host);
    });
  }

  private drainFrames(): void {
    while (this.readBuffer.length >= 4) {
      const frameLen = this.readBuffer.readUInt32LE(0);
      if (this.readBuffer.length < 4 + frameLen) break;

      const frameBody = this.readBuffer.slice(4, 4 + frameLen);
      this.readBuffer = this.readBuffer.slice(4 + frameLen);

      let decoded: unknown;
      try {
        decoded = unpack(frameBody);
      } catch {
        // Corrupt frame — drop connection.
        this.socket?.destroy();
        continue;
      }

      // Response is: [id, {Ok: wire_value} | {Err: string}]
      const resp = decoded as [number, Record<string, unknown>];
      const [id, resultEnv] = resp;
      const pend = this.pending.get(id);
      if (!pend) continue;
      this.pending.delete(id);

      if ('Ok' in resultEnv) {
        pend.resolve(fromWireValue(resultEnv.Ok));
      } else {
        pend.reject(new Error(String(resultEnv['Err'] ?? 'unknown server error')));
      }
    }
  }

  private async ensureConnected(): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;
    await this.connect();
  }

  /**
   * Send `cmd ARGS…` and return the response (plain JS value, not WireValue).
   */
  async execute(cmd: string, args: unknown[]): Promise<unknown> {
    await this.ensureConnected();

    const id = this.nextId++;
    const wireArgs = args.map(toWireValue);
    // Request struct as msgpack array: [id, command, args]
    const body = pack([id, cmd.toUpperCase(), wireArgs]);
    const lenBuf = Buffer.allocUnsafe(4);
    lenBuf.writeUInt32LE(body.length, 0);
    const frame = Buffer.concat([lenBuf, body]);

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket!.write(frame, (writeErr) => {
        if (writeErr) {
          this.pending.delete(id);
          reject(writeErr);
        }
      });
    });
  }

  close(): void {
    this.socket?.destroy();
    this.socket = null;
  }

  /**
   * Open a **dedicated** TCP connection for server-push pub/sub delivery.
   *
   * Sends a SUBSCRIBE frame on the dedicated socket, waits for the initial
   * acknowledgement (subscriber_id), then emits push frames (id === 0xFFFFFFFF)
   * to the returned `onMessage` callback until `cancel()` is called.
   *
   * Returns `{ subscriberId, cancel }`.
   */
  async subscribePush(
    topics: string[],
    onMessage: (msg: { topic: string; payload: unknown; id: string; timestamp: number }) => void,
  ): Promise<{ subscriberId: string; cancel: () => void }> {
    // Open a fresh dedicated socket.
    const sock = await new Promise<net.Socket>((resolve, reject) => {
      const s = new net.Socket();
      s.setTimeout(this.timeoutMs);
      s.once('connect', () => resolve(s));
      s.once('error', reject);
      s.on('timeout', () => s.destroy(new Error('subscribePush connect timeout')));
      s.connect(this.port, this.host);
    });

    // Send SUBSCRIBE frame.
    const id = this.nextId++;
    const wireArgs = topics.map(t => toWireValue(t));
    const body = pack([id, 'SUBSCRIBE', wireArgs]);
    const lenBuf = Buffer.allocUnsafe(4);
    lenBuf.writeUInt32LE(body.length, 0);
    sock.write(Buffer.concat([lenBuf, body]));

    // Read the initial response (subscriber_id etc.) then switch to push mode.
    let readBuf = Buffer.alloc(0);
    let subscriberId = '';
    let pushMode = false;
    let cancelled = false;

    const cancel = () => {
      cancelled = true;
      sock.destroy();
    };

    sock.on('data', (chunk: Buffer) => {
      if (cancelled) return;
      readBuf = Buffer.concat([readBuf, chunk]);

      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;

        const frameBody = readBuf.slice(4, 4 + frameLen);
        readBuf = readBuf.slice(4 + frameLen);

        let decoded: unknown;
        try { decoded = unpack(frameBody); } catch { continue; }

        const resp = decoded as [number, Record<string, unknown>];
        const [frameId, resultEnv] = resp;

        if (!pushMode) {
          // Initial SUBSCRIBE response — extract subscriber_id.
          if ('Ok' in resultEnv) {
            const val = fromWireValue(resultEnv.Ok);
            if (val && typeof val === 'object' && 'subscriber_id' in (val as object)) {
              subscriberId = String((val as Record<string, unknown>)['subscriber_id'] ?? '');
            }
          }
          pushMode = true;
          continue;
        }

        // Push frames carry id === 0xFFFFFFFF (u32::MAX).
        if (frameId === 0xFFFFFFFF && 'Ok' in resultEnv) {
          const val = fromWireValue(resultEnv.Ok) as Record<string, unknown>;
          if (val && typeof val === 'object') {
            const topic = String(val['topic'] ?? '');
            const payloadStr = val['payload'];
            let payload: unknown = payloadStr;
            if (typeof payloadStr === 'string') {
              try { payload = JSON.parse(payloadStr); } catch { payload = payloadStr; }
            }
            onMessage({
              topic,
              payload,
              id: String(val['id'] ?? ''),
              timestamp: Number(val['timestamp'] ?? 0),
            });
          }
        }
      }
    });

    sock.on('error', () => { /* connection closed */ });
    sock.on('close', () => { /* push stream ended */ });

    // Wait briefly for the initial SUBSCRIBE response before returning.
    await new Promise<void>(resolve => setTimeout(resolve, 50));

    return { subscriberId, cancel };
  }
}

// ── RESP3 transport ───────────────────────────────────────────────────────────

/**
 * Persistent TCP connection to a RESP3 (Redis-compatible) listener.
 *
 * Requests are sent sequentially (one at a time) to keep the parser simple.
 * A queue serialises concurrent callers.
 */
export class Resp3Transport {
  private readonly host: string;
  private readonly port: number;
  private readonly timeoutMs: number;
  private socket: net.Socket | null = null;
  private buffer: Buffer = Buffer.alloc(0);
  private waiter: (() => void) | null = null;
  private readonly queue: Array<() => void> = [];
  private busy = false;

  constructor(host: string, port: number, timeoutMs: number) {
    this.host = host;
    this.port = port;
    this.timeoutMs = timeoutMs;
  }

  private connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const sock = new net.Socket();
      sock.setTimeout(this.timeoutMs);

      sock.once('connect', () => {
        this.socket = sock;
        this.buffer = Buffer.alloc(0);
        resolve();
      });

      sock.on('data', (chunk: Buffer) => {
        this.buffer = Buffer.concat([this.buffer, chunk]);
        const cb = this.waiter;
        if (cb !== null) {
          this.waiter = null;
          cb();
        }
      });

      sock.on('error', (err) => {
        this.socket = null;
        reject(err);
      });

      sock.on('close', () => { this.socket = null; });
      sock.on('timeout', () => { sock.destroy(new Error('RESP3 timeout')); });

      sock.connect(this.port, this.host);
    });
  }

  private waitForData(): Promise<void> {
    return new Promise((resolve) => { this.waiter = resolve; });
  }

  private async readLine(): Promise<string> {
    while (true) {
      const nl = this.buffer.indexOf(0x0a); // '\n'
      if (nl !== -1) {
        const line = this.buffer.slice(0, nl + 1).toString('utf8');
        this.buffer = this.buffer.slice(nl + 1);
        return line;
      }
      await this.waitForData();
    }
  }

  private async readExact(n: number): Promise<Buffer> {
    // Bulk payloads are followed by CRLF; consume n + 2 bytes total.
    while (this.buffer.length < n + 2) {
      await this.waitForData();
    }
    const data = Buffer.from(this.buffer.slice(0, n));
    this.buffer = this.buffer.slice(n + 2);
    return data;
  }

  private async parseValue(): Promise<unknown> {
    const line = await this.readLine();
    const trimmed = line.replace(/\r?\n$/, '');
    const prefix = trimmed[0];
    const rest = trimmed.slice(1);

    switch (prefix) {
      case '+': return rest;                         // simple string
      case '-': throw new Error(rest);               // error
      case ':': return parseInt(rest, 10);           // integer
      case '_': return null;                         // null (RESP3)
      case '#': return rest === 't';                 // boolean (RESP3)
      case ',': {                                    // double (RESP3)
        if (rest === 'inf') return Infinity;
        if (rest === '-inf') return -Infinity;
        return parseFloat(rest);
      }
      case '$': {                                    // bulk string
        const len = parseInt(rest, 10);
        if (len < 0) return null;
        const data = await this.readExact(len);
        return data.toString('utf8');
      }
      case '*': {                                    // array
        const count = parseInt(rest, 10);
        if (count < 0) return null;
        const items: unknown[] = [];
        for (let i = 0; i < count; i++) items.push(await this.parseValue());
        return items;
      }
      case '%': {                                    // map (RESP3)
        const count = parseInt(rest, 10);
        const pairs: [unknown, unknown][] = [];
        for (let i = 0; i < count; i++) {
          const k = await this.parseValue();
          const v = await this.parseValue();
          pairs.push([k, v]);
        }
        return Object.fromEntries(pairs.map(([k, v]) => [String(k), v]));
      }
      case '~': {                                    // set (RESP3) → array
        const count = parseInt(rest, 10);
        const items: unknown[] = [];
        for (let i = 0; i < count; i++) items.push(await this.parseValue());
        return items;
      }
      default:
        throw new Error(`RESP3 unknown prefix: ${prefix}`);
    }
  }

  private async ensureConnected(): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;
    await this.connect();
  }

  /**
   * Enqueue a command for sequential execution.
   * Returns the raw parsed RESP3 value (string, number, null, or Array).
   */
  async execute(cmd: string, args: unknown[]): Promise<unknown> {
    return new Promise((resolve, reject) => {
      this.queue.push(async () => {
        try {
          await this.ensureConnected();

          // Build RESP2 multibulk frame: *N\r\n$len\r\nword\r\n…
          const parts: string[] = [cmd.toUpperCase(), ...args.map(String)];
          let frame = `*${parts.length}\r\n`;
          for (const p of parts) {
            frame += `$${Buffer.byteLength(p, 'utf8')}\r\n${p}\r\n`;
          }

          this.socket!.write(frame);
          const result = await this.parseValue();
          resolve(result);
        } catch (err) {
          reject(err instanceof Error ? err : new Error(String(err)));
        } finally {
          this.busy = false;
          if (this.queue.length > 0) {
            this.busy = true;
            const next = this.queue.shift()!;
            void next();
          }
        }
      });

      if (!this.busy) {
        this.busy = true;
        const next = this.queue.shift()!;
        void next();
      }
    });
  }

  close(): void {
    this.socket?.destroy();
    this.socket = null;
  }
}

// ── Command mapper ────────────────────────────────────────────────────────────

/**
 * Maps a dotted SDK command + JSON payload to a raw Redis-style command name
 * and an ordered argument list.
 *
 * Returns `null` for commands with no native mapping; the caller falls back to HTTP.
 */
export function mapCommand(
  cmd: string,
  payload: Record<string, unknown>,
): { rawCmd: string; args: unknown[] } | null {
  const s = (key: string): string => String(payload[key] ?? '');
  const n = (key: string, def: number): string => String(payload[key] ?? def);

  switch (cmd) {
    // ── KV ──────────────────────────────────────────────────────────────────
    case 'kv.get': return { rawCmd: 'GET', args: [s('key')] };

    case 'kv.set': {
      const args: unknown[] = [s('key'), payload['value'] ?? ''];
      if (payload['ttl'] != null) args.push('EX', String(payload['ttl']));
      return { rawCmd: 'SET', args };
    }

    case 'kv.del': return { rawCmd: 'DEL', args: [s('key')] };
    case 'kv.exists': return { rawCmd: 'EXISTS', args: [s('key')] };
    case 'kv.incr': return { rawCmd: 'INCR', args: [s('key')] };
    case 'kv.decr': return { rawCmd: 'DECR', args: [s('key')] };

    case 'kv.keys': {
      const prefix = String(payload['prefix'] ?? '');
      return { rawCmd: 'KEYS', args: [prefix ? `${prefix}*` : '*'] };
    }

    case 'kv.expire': return { rawCmd: 'EXPIRE', args: [s('key'), n('ttl', 0)] };
    case 'kv.ttl': return { rawCmd: 'TTL', args: [s('key')] };

    // ── Hash ─────────────────────────────────────────────────────────────────
    case 'hash.set': return { rawCmd: 'HSET', args: [s('key'), s('field'), s('value')] };
    case 'hash.get': return { rawCmd: 'HGET', args: [s('key'), s('field')] };
    case 'hash.getall': return { rawCmd: 'HGETALL', args: [s('key')] };
    case 'hash.del': return { rawCmd: 'HDEL', args: [s('key'), s('field')] };
    case 'hash.exists': return { rawCmd: 'HEXISTS', args: [s('key'), s('field')] };
    case 'hash.keys': return { rawCmd: 'HKEYS', args: [s('key')] };
    case 'hash.values': return { rawCmd: 'HVALS', args: [s('key')] };
    case 'hash.len': return { rawCmd: 'HLEN', args: [s('key')] };
    case 'hash.incrby': return { rawCmd: 'HINCRBY', args: [s('key'), s('field'), n('increment', 0)] };
    case 'hash.incrbyfloat': return { rawCmd: 'HINCRBYFLOAT', args: [s('key'), s('field'), n('increment', 0)] };
    case 'hash.setnx': return { rawCmd: 'HSETNX', args: [s('key'), s('field'), s('value')] };

    case 'hash.mset': {
      const args: unknown[] = [s('key')];
      const fields = payload['fields'];
      if (fields && typeof fields === 'object' && !Array.isArray(fields)) {
        // HashMap format
        for (const [k, v] of Object.entries(fields as Record<string, unknown>)) {
          args.push(k, String(v));
        }
      } else if (Array.isArray(fields)) {
        // Array format [{field, value}, ...]
        for (const item of fields as Array<Record<string, unknown>>) {
          args.push(String(item['field'] ?? ''), String(item['value'] ?? ''));
        }
      }
      return { rawCmd: 'HSET', args };
    }

    case 'hash.mget': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['fields'])) {
        args.push(...(payload['fields'] as string[]));
      }
      return { rawCmd: 'HMGET', args };
    }

    // ── List ─────────────────────────────────────────────────────────────────
    case 'list.lpush':
    case 'list.lpushx': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['values'])) args.push(...(payload['values'] as string[]));
      return { rawCmd: cmd === 'list.lpushx' ? 'LPUSHX' : 'LPUSH', args };
    }

    case 'list.rpush':
    case 'list.rpushx': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['values'])) args.push(...(payload['values'] as string[]));
      return { rawCmd: cmd === 'list.rpushx' ? 'RPUSHX' : 'RPUSH', args };
    }

    case 'list.lpop': {
      const args: unknown[] = [s('key')];
      if (payload['count'] != null) args.push(String(payload['count']));
      return { rawCmd: 'LPOP', args };
    }

    case 'list.rpop': {
      const args: unknown[] = [s('key')];
      if (payload['count'] != null) args.push(String(payload['count']));
      return { rawCmd: 'RPOP', args };
    }

    case 'list.range': return { rawCmd: 'LRANGE', args: [s('key'), n('start', 0), n('stop', -1)] };
    case 'list.len': return { rawCmd: 'LLEN', args: [s('key')] };
    case 'list.index': return { rawCmd: 'LINDEX', args: [s('key'), n('index', 0)] };
    case 'list.set': return { rawCmd: 'LSET', args: [s('key'), n('index', 0), String(payload['value'] ?? '')] };
    case 'list.trim': return { rawCmd: 'LTRIM', args: [s('key'), n('start', 0), n('end', -1)] };
    case 'list.rem': return { rawCmd: 'LREM', args: [s('key'), n('count', 0), String(payload['element'] ?? '')] };
    case 'list.rpoplpush': return { rawCmd: 'RPOPLPUSH', args: [s('key'), s('destination')] };
    case 'list.pos': return { rawCmd: 'LPOS', args: [s('key'), String(payload['element'] ?? '')] };

    case 'list.insert': {
      const before = payload['before'] !== false ? 'BEFORE' : 'AFTER';
      return { rawCmd: 'LINSERT', args: [s('key'), before, String(payload['pivot'] ?? ''), String(payload['value'] ?? '')] };
    }

    // ── Set ──────────────────────────────────────────────────────────────────
    case 'set.add': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      return { rawCmd: 'SADD', args };
    }

    case 'set.rem': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      return { rawCmd: 'SREM', args };
    }

    case 'set.ismember': return { rawCmd: 'SISMEMBER', args: [s('key'), s('member')] };
    case 'set.members': return { rawCmd: 'SMEMBERS', args: [s('key')] };
    case 'set.card': return { rawCmd: 'SCARD', args: [s('key')] };
    case 'set.pop': return { rawCmd: 'SPOP', args: [s('key'), n('count', 1)] };
    case 'set.randmember': return { rawCmd: 'SRANDMEMBER', args: [s('key'), n('count', 1)] };
    case 'set.move': return { rawCmd: 'SMOVE', args: [s('key'), s('destination'), s('member')] };

    case 'set.inter':
    case 'set.union':
    case 'set.diff': {
      const raw = { 'set.inter': 'SINTER', 'set.union': 'SUNION', 'set.diff': 'SDIFF' }[cmd]!;
      const args: unknown[] = Array.isArray(payload['keys']) ? [...(payload['keys'] as string[])] : [];
      return { rawCmd: raw, args };
    }

    case 'set.interstore':
    case 'set.unionstore':
    case 'set.diffstore': {
      const raw = { 'set.interstore': 'SINTERSTORE', 'set.unionstore': 'SUNIONSTORE', 'set.diffstore': 'SDIFFSTORE' }[cmd]!;
      const keys = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [];
      return { rawCmd: raw, args: [s('destination'), ...keys] };
    }

    // ── Sorted Set ────────────────────────────────────────────────────────────
    case 'sortedset.zadd': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) {
        for (const m of payload['members'] as Array<{ score: number; member: string }>) {
          args.push(String(m.score), m.member);
        }
      } else {
        args.push(String(payload['score'] ?? 0), String(payload['member'] ?? ''));
      }
      return { rawCmd: 'ZADD', args };
    }

    case 'sortedset.zrem': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      else args.push(String(payload['member'] ?? ''));
      return { rawCmd: 'ZREM', args };
    }

    case 'sortedset.zscore': return { rawCmd: 'ZSCORE', args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zcard': return { rawCmd: 'ZCARD', args: [s('key')] };
    case 'sortedset.zincrby': return { rawCmd: 'ZINCRBY', args: [s('key'), String(payload['increment'] ?? 0), String(payload['member'] ?? '')] };
    case 'sortedset.zrank': return { rawCmd: 'ZRANK', args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zrevrank': return { rawCmd: 'ZREVRANK', args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zcount': return { rawCmd: 'ZCOUNT', args: [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')] };

    case 'sortedset.zrange':
    case 'sortedset.zrevrange': {
      const raw = cmd === 'sortedset.zrevrange' ? 'ZREVRANGE' : 'ZRANGE';
      const args: unknown[] = [s('key'), n('start', 0), n('stop', -1)];
      if (payload['withscores']) args.push('WITHSCORES');
      return { rawCmd: raw, args };
    }

    case 'sortedset.zrangebyscore': {
      const args: unknown[] = [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')];
      if (payload['withscores']) args.push('WITHSCORES');
      return { rawCmd: 'ZRANGEBYSCORE', args };
    }

    case 'sortedset.zpopmin':
    case 'sortedset.zpopmax':
      return { rawCmd: cmd === 'sortedset.zpopmax' ? 'ZPOPMAX' : 'ZPOPMIN', args: [s('key'), n('count', 1)] };

    case 'sortedset.zremrangebyrank': return { rawCmd: 'ZREMRANGEBYRANK', args: [s('key'), n('start', 0), n('stop', -1)] };
    case 'sortedset.zremrangebyscore': return { rawCmd: 'ZREMRANGEBYSCORE', args: [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')] };

    case 'sortedset.zinterstore':
    case 'sortedset.zunionstore':
    case 'sortedset.zdiffstore': {
      const raw = { 'sortedset.zinterstore': 'ZINTERSTORE', 'sortedset.zunionstore': 'ZUNIONSTORE', 'sortedset.zdiffstore': 'ZDIFFSTORE' }[cmd]!;
      const keys = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [];
      return { rawCmd: raw, args: [s('destination'), String(keys.length), ...keys] };
    }

    // ── Queue ────────────────────────────────────────────────────────────────
    case 'queue.create': return {
      rawCmd: 'QCREATE',
      args: [
        s('name'),
        String(payload['max_depth'] ?? 0),
        String(payload['ack_deadline_secs'] ?? 30),
      ],
    };

    case 'queue.delete': return { rawCmd: 'QDELETE', args: [s('queue')] };
    case 'queue.list':   return { rawCmd: 'QLIST',   args: [] };
    case 'queue.purge':  return { rawCmd: 'QPURGE',  args: [s('queue')] };

    case 'queue.publish': {
      const pl = payload['payload'];
      let payloadArg: unknown;
      if (pl instanceof Uint8Array || Buffer.isBuffer(pl as unknown)) {
        payloadArg = pl;
      } else if (typeof pl === 'string') {
        payloadArg = pl;
      } else {
        payloadArg = JSON.stringify(pl ?? '');
      }
      return {
        rawCmd: 'QPUBLISH',
        args: [
          s('queue'),
          payloadArg,
          String(payload['priority'] ?? 0),
          String(payload['max_retries'] ?? 3),
        ],
      };
    }

    case 'queue.consume': return { rawCmd: 'QCONSUME', args: [s('queue'), s('consumer_id')] };
    case 'queue.ack':     return { rawCmd: 'QACK',     args: [s('queue'), s('message_id')] };
    case 'queue.nack':    return {
      rawCmd: 'QNACK',
      args: [s('queue'), s('message_id'), String(payload['requeue'] ?? true)],
    };
    case 'queue.stats':   return { rawCmd: 'QSTATS',   args: [s('queue')] };

    // ── Stream ───────────────────────────────────────────────────────────────
    case 'stream.create': return {
      rawCmd: 'SCREATE',
      args: [s('room'), String(payload['max_events'] ?? 0)],
    };
    case 'stream.delete': return { rawCmd: 'SDELETE', args: [s('room')] };
    case 'stream.list':   return { rawCmd: 'SLIST',   args: [] };
    case 'stream.publish': return {
      rawCmd: 'SPUBLISH',
      args: [s('room'), s('event'), JSON.stringify(payload['data'] ?? {})],
    };
    case 'stream.consume': return {
      rawCmd: 'SREAD',
      args: [s('room'), s('subscriber_id'), String(payload['from_offset'] ?? 0)],
    };
    case 'stream.stats': return { rawCmd: 'SSTATS', args: [s('room')] };

    // ── Pub/Sub ──────────────────────────────────────────────────────────────
    case 'pubsub.publish': return { rawCmd: 'PUBLISH', args: [s('topic'), JSON.stringify(payload['payload'] ?? payload['data'] ?? '')] };

    case 'pubsub.subscribe': {
      const topics = Array.isArray(payload['topics']) ? payload['topics'] as string[] : [];
      return { rawCmd: 'SUBSCRIBE', args: [...topics] };
    }

    case 'pubsub.unsubscribe': {
      const topics = Array.isArray(payload['topics']) ? payload['topics'] as string[] : [];
      return { rawCmd: 'UNSUBSCRIBE', args: [s('subscriber_id'), ...topics] };
    }

    case 'pubsub.topics':
    case 'pubsub.list': return { rawCmd: 'TOPICS', args: [] };

    // ── Transactions ─────────────────────────────────────────────────────────
    case 'transaction.multi':    return { rawCmd: 'MULTI',    args: [s('client_id')] };
    case 'transaction.exec':     return { rawCmd: 'EXEC',     args: [s('client_id')] };
    case 'transaction.discard':  return { rawCmd: 'DISCARD',  args: [s('client_id')] };
    case 'transaction.watch': {
      const keys = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [];
      return { rawCmd: 'WATCH', args: [s('client_id'), ...keys] };
    }
    case 'transaction.unwatch':  return { rawCmd: 'UNWATCH',  args: [s('client_id')] };

    // ── Scripts ──────────────────────────────────────────────────────────────
    case 'script.eval': {
      const keys = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [];
      const args = Array.isArray(payload['args']) ? payload['args'] as unknown[] : [];
      return { rawCmd: 'EVAL', args: [s('script'), String(keys.length), ...keys, ...args.map(String)] };
    }
    case 'script.evalsha': {
      const keys = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [];
      const args = Array.isArray(payload['args']) ? payload['args'] as unknown[] : [];
      return { rawCmd: 'EVALSHA', args: [s('sha1'), String(keys.length), ...keys, ...args.map(String)] };
    }
    case 'script.load':   return { rawCmd: 'SCRIPT.LOAD',   args: [s('script')] };
    case 'script.exists': {
      const hashes = Array.isArray(payload['hashes']) ? payload['hashes'] as string[] : [];
      return { rawCmd: 'SCRIPT.EXISTS', args: [...hashes] };
    }
    case 'script.flush':  return { rawCmd: 'SCRIPT.FLUSH',  args: [] };
    case 'script.kill':   return { rawCmd: 'SCRIPT.KILL',   args: [] };

    // ── HyperLogLog ──────────────────────────────────────────────────────────
    case 'hyperloglog.pfadd': {
      const elems = Array.isArray(payload['elements']) ? payload['elements'] as string[] : [];
      return { rawCmd: 'PFADD', args: [s('key'), ...elems] };
    }
    case 'hyperloglog.pfcount': {
      const keys2 = Array.isArray(payload['keys']) ? payload['keys'] as string[] : [s('key')];
      return { rawCmd: 'PFCOUNT', args: [...keys2] };
    }
    case 'hyperloglog.pfmerge': {
      const srcKeys = Array.isArray(payload['source_keys']) ? payload['source_keys'] as string[] : [];
      return { rawCmd: 'PFMERGE', args: [s('dest_key'), ...srcKeys] };
    }
    case 'hyperloglog.stats': return { rawCmd: 'HLLSTATS', args: [] };

    // ── Geospatial ───────────────────────────────────────────────────────────
    case 'geospatial.geoadd': {
      const members = Array.isArray(payload['members'])
        ? (payload['members'] as Array<{ lat: number; lon: number; member: string }>)
            .flatMap(m => [String(m.lat), String(m.lon), m.member])
        : [];
      return { rawCmd: 'GEOADD', args: [s('key'), ...members] };
    }
    case 'geospatial.geopos':   return { rawCmd: 'GEOPOS',   args: [s('key'), s('member')] };
    case 'geospatial.geodist':  return { rawCmd: 'GEODIST',  args: [s('key'), s('member1'), s('member2'), s('unit') || 'm'] };
    case 'geospatial.geohash':  return { rawCmd: 'GEOHASH',  args: [s('key'), s('member')] };
    case 'geospatial.georadius': return {
      rawCmd: 'GEORADIUS',
      args: [s('key'), n('longitude', 0), n('latitude', 0), n('radius', 0), s('unit') || 'm'],
    };
    case 'geospatial.georadiusbymember': return {
      rawCmd: 'GEORADIUSBYMEMBER',
      args: [s('key'), s('member'), n('radius', 0), s('unit') || 'm'],
    };
    case 'geospatial.geosearch': return {
      rawCmd: 'GEOSEARCH',
      args: [s('key'), 'FROMLONLAT', n('longitude', 0), n('latitude', 0), 'BYRADIUS', n('radius', 0), s('unit') || 'm'],
    };
    case 'geospatial.stats': return { rawCmd: 'GEOSTATS', args: [s('key')] };

    default:
      return null;
  }
}

// ── Response mapper ───────────────────────────────────────────────────────────

/**
 * Convert a raw protocol response into the JSON shape that SDK managers expect.
 * The raw value comes from RESP3 (JS primitives / arrays) or SynapRPC (fromWireValue).
 */
export function mapResponse(cmd: string, raw: unknown): unknown {
  const asInt = (v: unknown, def = 0): number => {
    if (typeof v === 'number') return v;
    if (typeof v === 'boolean') return v ? 1 : 0;
    return parseInt(String(v ?? def), 10);
  };
  const asFloat = (v: unknown, def = 0.0): number => typeof v === 'number' ? v : parseFloat(String(v ?? def));
  const asArr = (v: unknown): unknown[] => Array.isArray(v) ? v : v == null ? [] : [v];

  // Helper: convert interleaved [member, score, ...] array → [{member, score}, ...]
  const interleaved = (arr: unknown[]): Array<{ member: string; score: number }> => {
    const result = [];
    for (let i = 0; i + 1 < arr.length; i += 2) {
      result.push({ member: String(arr[i]), score: asFloat(arr[i + 1]) });
    }
    return result;
  };

  switch (cmd) {
    // KV
    case 'kv.get': return raw;
    case 'kv.set': return {};
    case 'kv.del': return { deleted: asInt(raw) > 0 };
    case 'kv.exists': return { exists: asInt(raw) > 0 };
    case 'kv.incr':
    case 'kv.decr': return { value: asInt(raw) };
    case 'kv.keys': return { keys: asArr(raw) };
    case 'kv.expire': return {};
    case 'kv.ttl': return raw;

    // Hash
    case 'hash.set': return { success: asInt(raw) >= 0 };
    case 'hash.get': return { value: raw ?? null };
    case 'hash.getall': {
      const arr = asArr(raw);
      const fields: Record<string, unknown> = {};
      for (let i = 0; i + 1 < arr.length; i += 2) {
        fields[String(arr[i])] = arr[i + 1];
      }
      return { fields };
    }
    case 'hash.del': return { deleted: asInt(raw) };
    case 'hash.exists': return { exists: asInt(raw) > 0 };
    case 'hash.keys': return { fields: asArr(raw) };
    case 'hash.values': return { values: asArr(raw) };
    case 'hash.len': return { length: asInt(raw) };
    case 'hash.mset': return { success: raw != null };
    case 'hash.mget': return { values: asArr(raw) };
    case 'hash.incrby': return { value: asInt(raw) };
    case 'hash.incrbyfloat': return { value: asFloat(raw) };
    case 'hash.setnx': return { created: asInt(raw) > 0 };

    // List
    case 'list.lpush':
    case 'list.rpush':
    case 'list.lpushx':
    case 'list.rpushx': return { length: asInt(raw) };
    case 'list.lpop':
    case 'list.rpop': return { values: raw == null ? [] : Array.isArray(raw) ? raw : [raw] };
    case 'list.range': return { values: asArr(raw) };
    case 'list.len': return { length: asInt(raw) };
    case 'list.index': return raw;
    case 'list.set':
    case 'list.trim': return {};
    case 'list.rem': return { count: asInt(raw) };
    case 'list.insert': return { length: asInt(raw) };
    case 'list.rpoplpush': return raw;
    case 'list.pos': return raw;

    // Set
    case 'set.add': return { added: asInt(raw) };
    case 'set.rem': return { removed: asInt(raw) };
    case 'set.ismember': return { is_member: asInt(raw) > 0 };
    case 'set.members':
    case 'set.pop':
    case 'set.randmember': return { members: asArr(raw) };
    case 'set.card': return { cardinality: asInt(raw) };
    case 'set.move': return { moved: asInt(raw) > 0 };
    case 'set.inter':
    case 'set.union':
    case 'set.diff': return { members: asArr(raw) };
    case 'set.interstore':
    case 'set.unionstore':
    case 'set.diffstore': return { count: asInt(raw) };

    // Sorted set
    case 'sortedset.zadd': return { added: asInt(raw) };
    case 'sortedset.zrem': return { removed: asInt(raw) };
    case 'sortedset.zscore': return { score: raw == null ? null : asFloat(raw) };
    case 'sortedset.zcard': return { count: asInt(raw) };
    case 'sortedset.zincrby': return { score: asFloat(raw) };
    case 'sortedset.zrank':
    case 'sortedset.zrevrank': return { rank: raw == null ? null : asInt(raw) };
    case 'sortedset.zcount':
    case 'sortedset.zremrangebyrank':
    case 'sortedset.zremrangebyscore': return { count: asInt(raw) };
    case 'sortedset.zrange':
    case 'sortedset.zrevrange':
    case 'sortedset.zrangebyscore': {
      const arr = asArr(raw);
      return { members: interleaved(arr) };
    }
    case 'sortedset.zpopmin':
    case 'sortedset.zpopmax': {
      const arr = asArr(raw);
      return { members: interleaved(arr) };
    }
    case 'sortedset.zinterstore':
    case 'sortedset.zunionstore':
    case 'sortedset.zdiffstore': return { count: asInt(raw) };

    // Queue
    case 'queue.create':
    case 'queue.delete':
    case 'queue.purge': return {};
    case 'queue.list': return Array.isArray(raw) ? raw : (raw == null ? [] : [raw]);
    case 'queue.publish': {
      if (raw && typeof raw === 'object' && 'message_id' in (raw as object)) return raw;
      return { message_id: String(raw ?? '') };
    }
    case 'queue.consume': return raw ?? null;
    case 'queue.ack':
    case 'queue.nack': return {};
    case 'queue.stats': return raw;

    // Stream
    case 'stream.create':
    case 'stream.delete': return {};
    case 'stream.list': return Array.isArray(raw) ? raw : (raw == null ? [] : [raw]);
    case 'stream.publish': {
      if (raw && typeof raw === 'object' && 'offset' in (raw as object)) return raw;
      return { offset: asInt(raw) };
    }
    case 'stream.consume': return Array.isArray(raw) ? { events: raw } : raw;
    case 'stream.stats': return raw;

    // Pub/Sub
    case 'pubsub.publish': {
      if (raw && typeof raw === 'object' && 'subscribers_matched' in (raw as object)) return raw;
      return { message_id: '', subscribers_matched: asInt(raw) };
    }
    case 'pubsub.subscribe': return raw;
    case 'pubsub.unsubscribe': return {};
    case 'pubsub.topics':
    case 'pubsub.list': return Array.isArray(raw) ? { topics: raw } : (raw ?? { topics: [] });

    // Transactions
    case 'transaction.multi':
    case 'transaction.discard':
    case 'transaction.watch':
    case 'transaction.unwatch': return raw ?? { success: true };
    case 'transaction.exec': return raw;

    // Scripts
    case 'script.eval':
    case 'script.evalsha': return raw;
    case 'script.load': {
      if (raw && typeof raw === 'object' && 'sha1' in (raw as object)) return raw;
      return { sha1: String(raw ?? '') };
    }
    case 'script.exists': {
      if (raw && typeof raw === 'object' && 'exists' in (raw as object)) return raw;
      return { exists: Array.isArray(raw) ? raw.map(Boolean) : [] };
    }
    case 'script.flush': return raw ?? { cleared: 0 };
    case 'script.kill': return raw ?? { terminated: false };

    // HyperLogLog
    case 'hyperloglog.pfadd': return { changed: asInt(raw) > 0 };
    case 'hyperloglog.pfcount': return { count: asInt(raw) };
    case 'hyperloglog.pfmerge': return {};
    case 'hyperloglog.stats': return raw;

    // Geospatial
    case 'geospatial.geoadd': return { added: asInt(raw) };
    case 'geospatial.geopos': return raw;
    case 'geospatial.geodist': return { distance: raw == null ? null : asFloat(raw) };
    case 'geospatial.geohash': return { hash: raw };
    case 'geospatial.georadius':
    case 'geospatial.georadiusbymember':
    case 'geospatial.geosearch': return { members: asArr(raw) };
    case 'geospatial.stats': return raw;

    default: return raw;
  }
}
