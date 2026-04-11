/**
 * Synap TypeScript SDK - SynapRPC Transport
 *
 * Persistent TCP connection using the SynapRPC binary protocol:
 *   wire frame = 4-byte LE length prefix  +  MessagePack body
 *
 * Request body (msgpack array):  [id: u32, command: string, args: WireValue[]]
 * Response body (msgpack array): [id: u32, {Ok: WireValue} | {Err: string}]
 *
 * WireValue encoding mirrors Rust's rmp_serde externally-tagged enum format:
 *   Null     → bare string  "Null"
 *   Str(x)   → {Str: x}
 *   Int(n)   → {Int: n}
 *   Float(f) → {Float: f}
 *   Bool(b)  → {Bool: b}
 *   Bytes(b) → {Bytes: b}
 *   Array(a) → {Array: [WireValue, ...]}
 *   Map(m)   → {Map: [[WireValue, WireValue], ...]}
 */

import * as net from 'net';
import { pack, unpack } from 'msgpackr';

// ── Wire value codec ──────────────────────────────────────────────────────────

/** Serde externally-tagged WireValue as produced/consumed by the Rust server. */
export type WireValue =
  | 'Null'
  | { Str: string }
  | { Int: number }
  | { Float: number }
  | { Bool: boolean }
  | { Bytes: Uint8Array | Buffer }
  | { Array: WireValue[] }
  | { Map: [WireValue, WireValue][] };

/**
 * Encode a plain JavaScript value as the externally-tagged WireValue envelope
 * expected by the Synap server.
 */
export function toWireValue(v: unknown): WireValue {
  if (v === null || v === undefined) return 'Null';
  if (typeof v === 'string') return { Str: v };
  if (typeof v === 'boolean') return { Bool: v };
  if (typeof v === 'number') {
    return Number.isInteger(v) ? { Int: v } : { Float: v };
  }
  if (v instanceof Uint8Array || Buffer.isBuffer(v)) return { Bytes: v };
  if (Array.isArray(v)) return { Array: v.map(toWireValue) };
  if (typeof v === 'object') {
    const pairs = Object.entries(v as Record<string, unknown>).map(
      ([k, val]): [WireValue, WireValue] => [{ Str: k }, toWireValue(val)],
    );
    return { Map: pairs };
  }
  return { Str: String(v) };
}

/**
 * Decode an externally-tagged WireValue envelope back to a plain JS value.
 * Bytes values that look like UTF-8 strings are decoded to string for SDK consumers.
 */
export function fromWireValue(wire: unknown): unknown {
  if (wire === 'Null' || wire === null || wire === undefined) return null;
  if (typeof wire === 'object') {
    const w = wire as Record<string, unknown>;
    if ('Str' in w) return w['Str'];
    if ('Int' in w) return w['Int'];
    if ('Float' in w) return w['Float'];
    if ('Bool' in w) return w['Bool'];
    if ('Bytes' in w) {
      const b = w['Bytes'];
      if (b instanceof Uint8Array || Buffer.isBuffer(b)) {
        return Buffer.from(b as Uint8Array).toString('utf8');
      }
      if (Array.isArray(b)) {
        return Buffer.from(b as number[]).toString('utf8');
      }
      return b;
    }
    if ('Array' in w) {
      return (w['Array'] as unknown[]).map(fromWireValue);
    }
    if ('Map' in w) {
      const pairs = w['Map'] as [unknown, unknown][];
      const obj: Record<string, unknown> = {};
      for (const [k, val] of pairs) {
        obj[String(fromWireValue(k))] = fromWireValue(val);
      }
      return obj;
    }
  }
  return wire;
}

// ── Pending request tracker ───────────────────────────────────────────────────

interface Pending {
  resolve: (value: unknown) => void;
  reject: (err: Error) => void;
}

// ── SynapRpcTransport ─────────────────────────────────────────────────────────

/** Maximum reconnect attempts before an execute call throws. */
const MAX_RECONNECT_ATTEMPTS = 2;

/**
 * Persistent, multiplexed TCP connection to the SynapRPC listener.
 *
 * - Lazy connect: the socket is opened on the first `execute()` call.
 * - Auto-reconnect: up to {@link MAX_RECONNECT_ATTEMPTS} attempts on failure.
 * - Multiplexing: concurrent requests are tracked by numeric request ID.
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

  // ── Internal connection management ─────────────────────────────────────────

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

      const frameBody = this.readBuffer.subarray(4, 4 + frameLen);
      this.readBuffer = this.readBuffer.subarray(4 + frameLen);

      let decoded: unknown;
      try {
        decoded = unpack(frameBody);
      } catch {
        // Corrupt frame — drop the connection so state resets cleanly.
        this.socket?.destroy();
        continue;
      }

      // Response: [id, {Ok: WireValue} | {Err: string}]
      const resp = decoded as [number, Record<string, unknown>];
      const [id, resultEnv] = resp;
      const pend = this.pending.get(id);
      if (!pend) continue;
      this.pending.delete(id);

      if ('Ok' in resultEnv) {
        pend.resolve(fromWireValue(resultEnv['Ok']));
      } else {
        pend.reject(new Error(String(resultEnv['Err'] ?? 'unknown server error')));
      }
    }
  }

  private async ensureConnected(attempt = 0): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;
    try {
      await this.connect();
    } catch (err) {
      if (attempt < MAX_RECONNECT_ATTEMPTS - 1) {
        await this.ensureConnected(attempt + 1);
      } else {
        throw err;
      }
    }
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /**
   * Execute `cmd` with `args` on the remote server and return the decoded response.
   *
   * @param cmd  Redis-style command name, e.g. `"SET"`, `"HGET"`.
   * @param args Command arguments as plain JS values; they are encoded as WireValues on the wire.
   * @returns    The decoded response (plain JS value, WireValue envelope stripped).
   */
  async execute(cmd: string, args: unknown[]): Promise<unknown> {
    await this.ensureConnected();

    const id = this.nextId++;
    const wireArgs = args.map(toWireValue);
    // Request msgpack array: [id, command, args]
    const body = pack([id, cmd.toUpperCase(), wireArgs]);
    const lenBuf = Buffer.allocUnsafe(4);
    lenBuf.writeUInt32LE(body.length, 0);
    const frame = Buffer.concat([lenBuf, body]);

    return new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket!.write(frame, (writeErr) => {
        if (writeErr) {
          this.pending.delete(id);
          reject(writeErr);
        }
      });
    });
  }

  /**
   * Close the persistent TCP socket and reject all pending requests.
   */
  close(): void {
    this.socket?.destroy();
    this.socket = null;
  }

  /**
   * Open a **dedicated** TCP connection for server-push pub/sub delivery.
   *
   * Sends a SUBSCRIBE frame on a new socket, waits for the initial
   * acknowledgement (subscriber_id), then relays push frames (id === 0xFFFFFFFF)
   * to `onMessage` until `cancel()` is called.
   *
   * @returns `{ subscriberId, cancel }` — call `cancel()` to tear down the push socket.
   */
  async subscribePush(
    topics: string[],
    onMessage: (msg: {
      topic: string;
      payload: unknown;
      id: string;
      timestamp: number;
    }) => void,
  ): Promise<{ subscriberId: string; cancel: () => void }> {
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
    const wireArgs = topics.map(toWireValue);
    const body = pack([id, 'SUBSCRIBE', wireArgs]);
    const lenBuf = Buffer.allocUnsafe(4);
    lenBuf.writeUInt32LE(body.length, 0);
    sock.write(Buffer.concat([lenBuf, body]));

    let readBuf = Buffer.alloc(0);
    let subscriberId = '';
    let pushMode = false;
    let cancelled = false;

    const cancel = (): void => {
      cancelled = true;
      sock.destroy();
    };

    sock.on('data', (chunk: Buffer) => {
      if (cancelled) return;
      readBuf = Buffer.concat([readBuf, chunk]);

      while (readBuf.length >= 4) {
        const frameLen = readBuf.readUInt32LE(0);
        if (readBuf.length < 4 + frameLen) break;

        const frameBody = readBuf.subarray(4, 4 + frameLen);
        readBuf = readBuf.subarray(4 + frameLen);

        let decoded: unknown;
        try {
          decoded = unpack(frameBody);
        } catch {
          continue;
        }

        const resp = decoded as [number, Record<string, unknown>];
        const [frameId, resultEnv] = resp;

        if (!pushMode) {
          // Initial SUBSCRIBE response — capture subscriber_id.
          if ('Ok' in resultEnv) {
            const val = fromWireValue(resultEnv['Ok']);
            if (val && typeof val === 'object' && 'subscriber_id' in (val as object)) {
              subscriberId = String(
                (val as Record<string, unknown>)['subscriber_id'] ?? '',
              );
            }
          }
          pushMode = true;
          continue;
        }

        // Push frames carry id === 0xFFFFFFFF (u32::MAX).
        if (frameId === 0xffffffff && 'Ok' in resultEnv) {
          const val = fromWireValue(resultEnv['Ok']) as Record<string, unknown>;
          if (val && typeof val === 'object') {
            const topic = String(val['topic'] ?? '');
            const payloadRaw = val['payload'];
            let payload: unknown = payloadRaw;
            if (typeof payloadRaw === 'string') {
              try {
                payload = JSON.parse(payloadRaw);
              } catch {
                payload = payloadRaw;
              }
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

    sock.on('error', () => {
      /* push stream error — connection closed */
    });
    sock.on('close', () => {
      /* push stream ended */
    });

    // Allow time for the initial SUBSCRIBE acknowledgement.
    await new Promise<void>((resolve) => setTimeout(resolve, 50));

    return { subscriberId, cancel };
  }
}
