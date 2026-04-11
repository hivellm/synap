/**
 * Synap TypeScript SDK - RESP3 Transport
 *
 * Persistent TCP connection using the Redis Serialization Protocol (RESP2/RESP3).
 *
 * Outgoing wire format — RESP2 multibulk (compatible with Synap's RESP listener):
 *   *N\r\n
 *   $<byteLen>\r\n<arg>\r\n   (repeated N times)
 *
 * Incoming parsing handles all RESP2 and RESP3 type prefixes:
 *   +   simple string
 *   -   error
 *   :   integer
 *   $   bulk string
 *   *   array
 *   _   null (RESP3)
 *   #   boolean (RESP3)
 *   ,   double (RESP3)
 *   %   map (RESP3)
 *   ~   set (RESP3) — returned as an array
 *
 * Requests are serialised (one at a time) with an internal queue so the
 * response parser stays simple.
 */

import * as net from 'net';

/**
 * Persistent, sequentially-executing TCP connection to a RESP3-compatible listener.
 */
export class Resp3Transport {
  private readonly host: string;
  private readonly port: number;
  private readonly timeoutMs: number;

  private socket: net.Socket | null = null;
  private buffer: Buffer = Buffer.alloc(0);
  private dataWaiter: (() => void) | null = null;
  private readonly queue: Array<() => void> = [];
  private busy = false;

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
        this.buffer = Buffer.alloc(0);
        resolve();
      });

      sock.on('data', (chunk: Buffer) => {
        this.buffer = Buffer.concat([this.buffer, chunk]);
        const waiter = this.dataWaiter;
        if (waiter !== null) {
          this.dataWaiter = null;
          waiter();
        }
      });

      sock.on('error', (err) => {
        this.socket = null;
        reject(err);
      });

      sock.on('close', () => {
        this.socket = null;
      });

      sock.on('timeout', () => {
        sock.destroy(new Error('RESP3 connection timeout'));
      });

      sock.connect(this.port, this.host);
    });
  }

  private waitForData(): Promise<void> {
    return new Promise((resolve) => {
      this.dataWaiter = resolve;
    });
  }

  private async readLine(): Promise<string> {
    while (true) {
      const nl = this.buffer.indexOf(0x0a); // '\n'
      if (nl !== -1) {
        const line = this.buffer.subarray(0, nl + 1).toString('utf8');
        this.buffer = this.buffer.subarray(nl + 1);
        return line;
      }
      await this.waitForData();
    }
  }

  /**
   * Read exactly `n` bytes followed by the mandatory CRLF terminator.
   * Consumes `n + 2` bytes from the internal buffer.
   */
  private async readExact(n: number): Promise<Buffer> {
    while (this.buffer.length < n + 2) {
      await this.waitForData();
    }
    const data = Buffer.from(this.buffer.subarray(0, n));
    this.buffer = this.buffer.subarray(n + 2);
    return data;
  }

  /** Recursively parse one RESP2/RESP3 value from the socket buffer. */
  private async parseValue(): Promise<unknown> {
    const line = await this.readLine();
    const trimmed = line.replace(/\r?\n$/, '');
    const prefix = trimmed[0];
    const rest = trimmed.slice(1);

    switch (prefix) {
      case '+': return rest;                      // simple string
      case '-': throw new Error(rest);            // error
      case ':': return parseInt(rest, 10);        // integer
      case '_': return null;                      // null (RESP3)
      case '#': return rest === 't';             // boolean (RESP3)
      case ',': {                                 // double (RESP3)
        if (rest === 'inf') return Infinity;
        if (rest === '-inf') return -Infinity;
        return parseFloat(rest);
      }
      case '$': {                                 // bulk string
        const len = parseInt(rest, 10);
        if (len < 0) return null;
        const data = await this.readExact(len);
        return data.toString('utf8');
      }
      case '*': {                                 // array
        const count = parseInt(rest, 10);
        if (count < 0) return null;
        const items: unknown[] = [];
        for (let i = 0; i < count; i++) {
          items.push(await this.parseValue());
        }
        return items;
      }
      case '%': {                                 // map (RESP3)
        const count = parseInt(rest, 10);
        const result: Record<string, unknown> = {};
        for (let i = 0; i < count; i++) {
          const k = await this.parseValue();
          const v = await this.parseValue();
          result[String(k)] = v;
        }
        return result;
      }
      case '~': {                                 // set (RESP3) → array
        const count = parseInt(rest, 10);
        const items: unknown[] = [];
        for (let i = 0; i < count; i++) {
          items.push(await this.parseValue());
        }
        return items;
      }
      default:
        throw new Error(`RESP3: unknown prefix '${prefix}' in response`);
    }
  }

  private async ensureConnected(): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;
    await this.connect();
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /**
   * Enqueue `cmd` for sequential execution.
   *
   * @param cmd  Redis-style command name, e.g. `"SET"`, `"HGET"`.
   * @param args Command arguments (stringified before sending).
   * @returns    The raw parsed RESP value (string | number | null | unknown[]).
   */
  execute(cmd: string, args: unknown[]): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const run = async (): Promise<void> => {
        try {
          await this.ensureConnected();

          // Build RESP2 multibulk frame.
          const parts = [cmd.toUpperCase(), ...args.map(String)];
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
            next();
          }
        }
      };

      this.queue.push(run);
      if (!this.busy) {
        this.busy = true;
        const next = this.queue.shift()!;
        next();
      }
    });
  }

  /**
   * Close the persistent TCP socket.
   */
  close(): void {
    this.socket?.destroy();
    this.socket = null;
  }
}
