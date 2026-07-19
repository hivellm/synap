/**
 * Synap TypeScript SDK — SynapRPC transport.
 *
 * The wire layer is **not implemented here**. It is
 * [Thunder](https://github.com/hivellm/thunder) (`@hivehub/thunder`) — the
 * HiveLLM family's shared binary RPC client, the same protocol the Synap
 * server runs on, so the two ends cannot drift.
 *
 * What Thunder brings that the hand-written transport did not:
 *
 * - the frame cap validated against the length prefix **before** allocating,
 *   closing an unbounded-allocation hole that a remote peer could trigger;
 * - a real handshake, so the SDK can authenticate on the RPC port;
 * - connect and per-call timeouts, and lazy reconnect with capped retries;
 * - a push hook, replacing the previous `setTimeout(50)` guess at when the
 *   SUBSCRIBE acknowledgement had arrived.
 *
 * What stays here is Synap's own: the plain-JS ↔ wire value conversion the
 * SDK's command mappers speak.
 */

import {
  Client,
  Config,
  Value,
  type ClientOptions,
  type Credentials,
} from '@hivehub/thunder';

// ── Wire values ───────────────────────────────────────────────────────────────

/**
 * The wire value model, re-exported from Thunder.
 *
 * Previously a hand-written mirror of Rust's externally-tagged serde encoding
 * (`'Null' | {Str: string} | …`). Thunder owns the encoding now, so this is its
 * discriminated union (`{kind: 'str', value: string}`), and the tagging details
 * are no longer the SDK's business.
 */
export type WireValue = Value;

/** Largest integer that survives a `bigint` → `number` round-trip intact. */
const MAX_SAFE = BigInt(Number.MAX_SAFE_INTEGER);
const MIN_SAFE = -MAX_SAFE;

/**
 * Encode a plain JavaScript value as a Thunder {@link Value}.
 *
 * Integers become `Int`, other numbers `Float`, byte arrays `Bytes`, plain
 * objects an ordered `Map` keyed by string — matching what the Synap server's
 * dispatch tree expects for each argument position.
 */
export function toWireValue(v: unknown): WireValue {
  if (v === null || v === undefined) return Value.null();
  if (typeof v === 'string') return Value.str(v);
  if (typeof v === 'boolean') return Value.bool(v);
  if (typeof v === 'bigint') return Value.int(v);
  if (typeof v === 'number') {
    return Number.isInteger(v) ? Value.int(v) : Value.float(v);
  }
  if (v instanceof Uint8Array) return Value.bytes(v);
  if (Buffer.isBuffer(v)) return Value.bytes(new Uint8Array(v));
  if (Array.isArray(v)) return Value.array(v.map(toWireValue));
  if (typeof v === 'object') {
    const pairs = Object.entries(v as Record<string, unknown>).map(
      ([k, val]): [WireValue, WireValue] => [Value.str(k), toWireValue(val)],
    );
    return Value.map(pairs);
  }
  return Value.str(String(v));
}

/**
 * Decode a Thunder {@link Value} back to a plain JS value.
 *
 * `Bytes` decode as UTF-8 when they are valid UTF-8 -- Synap's SDK surface is
 * string-oriented -- and stay a `Buffer` when they are not. Decoding
 * unconditionally replaced every invalid sequence with U+FFFD, so a binary
 * value came back corrupted and unrecoverable: `deadbeef` read back as
 * `adfdfd`.
 *
 * `Int` narrows to `number` when it fits safely and stays a `bigint` when it
 * does not, so a value outside ±2^53 is never silently corrupted.
 */
const UTF8_STRICT = new TextDecoder('utf-8', { fatal: true });
export function fromWireValue(wire: unknown): unknown {
  const value = wire as Value | undefined;
  if (value === null || value === undefined) return null;

  switch (value.kind) {
    case 'null':
      return null;
    case 'str':
      return value.value;
    case 'bool':
      return value.value;
    case 'float':
      return value.value;
    case 'int':
      return value.value >= MIN_SAFE && value.value <= MAX_SAFE
        ? Number(value.value)
        : value.value;
    case 'bytes':
      try {
        return UTF8_STRICT.decode(value.value);
      } catch {
        return Buffer.from(value.value);
      }
    case 'array':
      return value.value.map(fromWireValue);
    case 'map': {
      const obj: Record<string, unknown> = {};
      for (const [k, val] of value.value) {
        obj[String(fromWireValue(k))] = fromWireValue(val);
      }
      return obj;
    }
    default:
      return wire;
  }
}

// ── Protocol configuration ────────────────────────────────────────────────────

/**
 * How Synap uses the Thunder wire, mirroring the server's `synap_config()`.
 *
 * Thunder ships one standard and zero product knowledge, so this description
 * lives in Synap's own repository. Every divergence from `Config.standard()` is
 * explicit: Synap authenticates with `AUTH` rather than a mandatory `HELLO`, it
 * ships a push-producing command (`SUBSCRIBE`), its errors use the
 * Redis-compatible prefixes it shares with its RESP3 port, and its frame cap is
 * 512 MiB rather than 64.
 */
export function synapConfig(): Config {
  return Config.standard()
    .withScheme('synap')
    .withPort(15_501)
    .withHandshake('auth_command')
    .withHelloStyle('not_used')
    .withPush('enabled')
    .withErrorCodes('resp3_prefixes')
    .withMaxFrameBytes(512 * 1024 * 1024);
}

/** Credentials for the RPC handshake, resolved from the SDK's auth options. */
export interface RpcCredentials {
  username?: string;
  password?: string;
  apiKey?: string;
}

function toThunderCredentials(creds: RpcCredentials | undefined): Credentials | undefined {
  if (!creds) return undefined;
  if (creds.apiKey) return { type: 'apiKey', apiKey: creds.apiKey };
  if (creds.username && creds.password) {
    return { type: 'userPass', user: creds.username, pass: creds.password };
  }
  return undefined;
}

// ── SynapRpcTransport ─────────────────────────────────────────────────────────

/**
 * Persistent, multiplexed TCP connection to the SynapRPC listener.
 *
 * Lazily connected on the first `execute()`; concurrent calls pipeline over the
 * single connection and are demultiplexed by frame id.
 */
export class SynapRpcTransport {
  private readonly endpoint: string;
  private readonly options: ClientOptions;

  private client: Client | null = null;
  /** In-flight connect, so concurrent first calls dial exactly once. */
  private connecting: Promise<Client> | null = null;

  constructor(host: string, port: number, timeoutMs: number, credentials?: RpcCredentials) {
    this.endpoint = `synap://${host}:${port}`;
    this.options = {
      connectTimeoutMs: timeoutMs,
      callTimeoutMs: timeoutMs,
      clientName: 'synap-typescript-sdk',
      credentials: toThunderCredentials(credentials),
    };
  }

  /** Dial a fresh Thunder client against the configured endpoint. */
  private dial(): Promise<Client> {
    return Client.connect(this.endpoint, synapConfig(), this.options);
  }

  /** The shared client, dialed once on first use. */
  private async ensureClient(): Promise<Client> {
    if (this.client) return this.client;
    if (!this.connecting) {
      this.connecting = this.dial()
        .then((client) => {
          this.client = client;
          return client;
        })
        .finally(() => {
          this.connecting = null;
        });
    }
    return this.connecting;
  }

  /**
   * Execute `cmd` with `args` and return the decoded response.
   *
   * @param cmd  Redis-style command name, e.g. `"SET"`, `"HGET"`.
   * @param args Command arguments as plain JS values.
   * @returns    The decoded response as a plain JS value.
   */
  async execute(cmd: string, args: unknown[]): Promise<unknown> {
    const client = await this.ensureClient();
    const result = await client.call(cmd.toUpperCase(), args.map(toWireValue));
    return fromWireValue(result);
  }

  /** Close the connection and fail anything still in flight. */
  close(): void {
    const client = this.client;
    this.client = null;
    void client?.close();
  }

  /**
   * Open a **dedicated** connection for server-push pub/sub delivery.
   *
   * The push hook is registered before `SUBSCRIBE` is sent, so a message
   * published between the server's acknowledgement and the registration cannot
   * slip past — the previous implementation waited 50 ms and hoped.
   *
   * @returns `{ subscriberId, cancel }` — call `cancel()` to tear the push
   *          connection down.
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
    const client = await this.dial();

    client.onPush((value: Value) => {
      const frame = fromWireValue(value) as Record<string, unknown> | null;
      if (!frame || typeof frame !== 'object') return;

      // The server encodes the payload as a JSON string; hand callers the
      // parsed value when it parses, and the raw string when it does not.
      const rawPayload = frame['payload'];
      let payload: unknown = rawPayload;
      if (typeof rawPayload === 'string') {
        try {
          payload = JSON.parse(rawPayload);
        } catch {
          payload = rawPayload;
        }
      }

      onMessage({
        topic: String(frame['topic'] ?? ''),
        payload,
        id: String(frame['id'] ?? ''),
        timestamp: Number(frame['timestamp'] ?? 0),
      });
    });

    const result = await client.call(
      'SUBSCRIBE',
      topics.map((t) => Value.str(t)),
    );
    const subscriberId = Value.asStr(Value.mapGet(result, 'subscriber_id')) ?? '';

    return {
      subscriberId,
      cancel: (): void => {
        void client.close();
      },
    };
  }

  /**
   * Open a **dedicated** connection driven by `KV.WATCH` — the watch twin of
   * {@link subscribePush}. Envelopes arrive as parsed objects via `onEvent`.
   *
   * @returns `{ subscriberId, cancel }` — `cancel()` issues `KV.UNWATCH` on
   *          the same connection (best-effort) and tears it down.
   */
  async watchPush(
    pattern: string,
    mode: 'value' | 'notify',
    onEvent: (envelope: Record<string, unknown>) => void,
  ): Promise<{ subscriberId: string; cancel: () => void }> {
    const client = await this.dial();

    // Register the hook before KV.WATCH, so an event published between the
    // server's acknowledgement and the registration cannot slip past.
    client.onPush((value: Value) => {
      const frame = fromWireValue(value) as Record<string, unknown> | null;
      if (!frame || typeof frame !== 'object') return;

      // The bridge encodes the envelope as a JSON string.
      const rawPayload = frame['payload'];
      let envelope: unknown = rawPayload;
      if (typeof rawPayload === 'string') {
        try {
          envelope = JSON.parse(rawPayload);
        } catch {
          return; // not a watch envelope
        }
      }
      if (envelope && typeof envelope === 'object') {
        onEvent(envelope as Record<string, unknown>);
      }
    });

    const args = mode === 'notify' ? [Value.str(pattern), Value.str('notify')] : [Value.str(pattern)];
    const result = await client.call('KV.WATCH', args);
    const subscriberId = Value.asStr(Value.mapGet(result, 'subscriber_id')) ?? '';

    return {
      subscriberId,
      cancel: (): void => {
        // Teardown issues KV.UNWATCH before the socket closes, so the server
        // drops the routing entry promptly.
        void client
          .call('KV.UNWATCH', [Value.str(subscriberId)])
          .catch(() => undefined)
          .finally(() => void client.close());
      },
    };
  }
}
