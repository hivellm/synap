/**
 * Synap TypeScript SDK - Base Client
 *
 * Core client supporting three transports:
 *  - SynapRPC (default): MessagePack-framed binary TCP
 *  - RESP3: Redis-compatible text TCP
 *  - HTTP: original StreamableHTTP REST
 *
 * Mapped commands route through the native transport; anything unmapped
 * (queues, streams, pub/sub, scripting, …) falls back to HTTP.
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  SynapRequest,
  SynapResponse,
  SynapClientOptions,
  AuthOptions,
} from './types';
import { NetworkError, ServerError, TimeoutError, UnsupportedCommandError } from './types';
import {
  SynapRpcTransport,
  Resp3Transport,
  mapCommand,
  mapResponse,
} from './transport';
import type { TransportMode } from './transport';

// ── Internal sealed transport discriminant ────────────────────────────────────

type InternalTransport =
  | { kind: 'http' }
  | { kind: 'synaprpc'; impl: SynapRpcTransport }
  | { kind: 'resp3'; impl: Resp3Transport };

// ── SynapClient ────────────────────────────────────────────────────────────────

/**
 * Base client for Synap server communication.
 */
export class SynapClient {
  private readonly baseUrl: string;
  private readonly timeout: number;
  private readonly debug: boolean;
  private readonly auth?: AuthOptions;
  private readonly transport: InternalTransport;

  constructor(options: SynapClientOptions = {}) {
    this.timeout = options.timeout ?? 30_000;
    this.debug = options.debug ?? false;
    this.auth = options.auth;

    // ── URL-scheme-based transport inference (v0.11.0+) ──────────────────────
    //
    // Resolution order:
    //   1. synap://host:port  → SynapRPC (binary TCP, port 15501)
    //   2. resp3://host:port  → RESP3 (text TCP, port 6379)
    //   3. http[s]://host     → HTTP REST (port 15500)
    //   4. no URL given       → SynapRPC on 127.0.0.1:15501 (DEFAULT)
    //
    // The legacy `transport`, `rpcHost`, `rpcPort`, `resp3Host`, `resp3Port`
    // fields on `SynapClientOptions` remain supported for backward compatibility.

    const rawUrl = options.url ?? '';

    if (rawUrl.startsWith('synap://')) {
      const [host, port] = SynapClient.parseHostPort(rawUrl.slice('synap://'.length), 15_501);
      this.baseUrl = `http://${host}:15500`;
      this.transport = {
        kind: 'synaprpc',
        impl: new SynapRpcTransport(host, port, this.timeout),
      };
    } else if (rawUrl.startsWith('resp3://')) {
      const [host, port] = SynapClient.parseHostPort(rawUrl.slice('resp3://'.length), 6_379);
      this.baseUrl = `http://${host}:15500`;
      this.transport = {
        kind: 'resp3',
        impl: new Resp3Transport(host, port, this.timeout),
      };
    } else if (rawUrl.startsWith('http://') || rawUrl.startsWith('https://')) {
      // Explicit HTTP URL — honour the legacy transport field if set, else HTTP.
      this.baseUrl = rawUrl;
      const mode: TransportMode = options.transport ?? 'http';
      switch (mode) {
        case 'synaprpc':
          this.transport = {
            kind: 'synaprpc',
            impl: new SynapRpcTransport(
              options.rpcHost ?? '127.0.0.1',
              options.rpcPort ?? 15_501,
              this.timeout,
            ),
          };
          break;
        case 'resp3':
          this.transport = {
            kind: 'resp3',
            impl: new Resp3Transport(
              options.resp3Host ?? '127.0.0.1',
              options.resp3Port ?? 6_379,
              this.timeout,
            ),
          };
          break;
        default:
          this.transport = { kind: 'http' };
      }
    } else if (rawUrl === '' && options.transport == null) {
      // No URL and no explicit transport override → SynapRPC is the default.
      const host = options.rpcHost ?? '127.0.0.1';
      const port = options.rpcPort ?? 15_501;
      this.baseUrl = `http://${host}:15500`;
      this.transport = {
        kind: 'synaprpc',
        impl: new SynapRpcTransport(host, port, this.timeout),
      };
    } else {
      // Legacy: no URL but explicit options.transport override, or an unrecognised
      // URL that we treat as an HTTP base URL for backward compatibility.
      this.baseUrl = rawUrl || 'http://localhost:15500';
      const mode: TransportMode = options.transport ?? 'http';
      switch (mode) {
        case 'synaprpc':
          this.transport = {
            kind: 'synaprpc',
            impl: new SynapRpcTransport(
              options.rpcHost ?? '127.0.0.1',
              options.rpcPort ?? 15_501,
              this.timeout,
            ),
          };
          break;
        case 'resp3':
          this.transport = {
            kind: 'resp3',
            impl: new Resp3Transport(
              options.resp3Host ?? '127.0.0.1',
              options.resp3Port ?? 6_379,
              this.timeout,
            ),
          };
          break;
        default:
          this.transport = { kind: 'http' };
      }
    }
  }

  /** Parse `"host:port"` from a URL authority string. */
  private static parseHostPort(authority: string, defaultPort: number): [string, number] {
    // Strip trailing path components.
    const auth = authority.split('/')[0] ?? authority;
    const colon = auth.lastIndexOf(':');
    if (colon !== -1) {
      const host = auth.slice(0, colon);
      const port = parseInt(auth.slice(colon + 1), 10);
      return [host, Number.isFinite(port) ? port : defaultPort];
    }
    return [auth, defaultPort];
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /**
   * Send a command to the Synap server.
   *
   * If the active transport is native (SynapRPC or RESP3) and the command has
   * a mapping, the native protocol is used. Otherwise falls through to HTTP.
   */
  async sendCommand<T = any>(
    command: string,
    payload: Record<string, unknown> = {},
  ): Promise<T> {
    if (this.transport.kind !== 'http') {
      const mapped = mapCommand(command, payload);
      if (mapped) {
        if (this.debug) {
          console.log('[Synap] Native transport:', command, mapped.rawCmd, mapped.args);
        }
        try {
          const raw =
            this.transport.kind === 'synaprpc'
              ? await this.transport.impl.execute(mapped.rawCmd, mapped.args)
              : await (this.transport as { kind: 'resp3'; impl: Resp3Transport }).impl.execute(
                  mapped.rawCmd,
                  mapped.args,
                );
          const result = mapResponse(command, raw) as T;
          if (this.debug) {
            console.log('[Synap] Native response:', result);
          }
          return result;
        } catch (err) {
          // Surface errors from native transport as NetworkError so callers can
          // catch them consistently.
          if (err instanceof Error) {
            throw new NetworkError(`SynapRPC error: ${err.message}`, err);
          }
          throw new NetworkError('SynapRPC unknown error');
        }
      }
      // Unmapped command on a native transport → UnsupportedCommandError.
      // No silent HTTP fallback.
      throw new UnsupportedCommandError(command, this.transport.kind);
    }

    return this.sendHttp<T>(command, payload);
  }

  /**
   * Return the `SynapRpcTransport` instance when using the `synap://` URL scheme,
   * or `null` for other transports.
   *
   * Used internally by reactive pub/sub to open dedicated push connections.
   */
  synapRpcTransport(): SynapRpcTransport | null {
    return this.transport.kind === 'synaprpc' ? this.transport.impl : null;
  }

  /**
   * Ping the server to check connectivity.
   */
  async ping(): Promise<boolean> {
    try {
      const response = await fetch(`${this.baseUrl}/health`, {
        signal: AbortSignal.timeout(this.timeout),
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  /**
   * Get server health status.
   */
  async health(): Promise<{ status: string; service: string; version: string }> {
    const response = await fetch(`${this.baseUrl}/health`, {
      signal: AbortSignal.timeout(this.timeout),
    });

    if (!response.ok) {
      throw new NetworkError('Health check failed');
    }

    return response.json() as Promise<{ status: string; service: string; version: string }>;
  }

  /**
   * Close persistent TCP connections (SynapRPC / RESP3).
   */
  close(): void {
    if (this.transport.kind === 'synaprpc') {
      this.transport.impl.close();
    } else if (this.transport.kind === 'resp3') {
      this.transport.impl.close();
    }
  }

  // ── Private HTTP implementation ─────────────────────────────────────────────

  private async sendHttp<T = any>(
    command: string,
    payload: Record<string, unknown>,
  ): Promise<T> {
    const request: SynapRequest = {
      command,
      request_id: uuidv4(),
      payload,
    };

    if (this.debug) {
      console.log('[Synap] HTTP Request:', JSON.stringify(request, null, 2));
    }

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), this.timeout);

      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        Accept: 'application/json',
        'Accept-Encoding': 'gzip',
      };

      if (this.auth) {
        if (this.auth.type === 'basic' && this.auth.username && this.auth.password) {
          const credentials = btoa(`${this.auth.username}:${this.auth.password}`);
          headers['Authorization'] = `Basic ${credentials}`;
        } else if (this.auth.type === 'api_key' && this.auth.apiKey) {
          headers['Authorization'] = `Bearer ${this.auth.apiKey}`;
        }
      }

      const response = await fetch(`${this.baseUrl}/api/v1/command`, {
        method: 'POST',
        headers,
        body: JSON.stringify(request),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new ServerError(
          `HTTP ${response.status}: ${response.statusText}`,
          response.status,
          request.request_id,
        );
      }

      const data = (await response.json()) as SynapResponse<T>;

      if (this.debug) {
        console.log('[Synap] HTTP Response:', JSON.stringify(data, null, 2));
      }

      if (!data.success) {
        throw new ServerError(
          data.error ?? 'Unknown server error',
          undefined,
          data.request_id,
        );
      }

      return data.payload as T;
    } catch (error) {
      if (error instanceof ServerError) throw error;

      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          throw new TimeoutError(`Request timed out after ${this.timeout}ms`, this.timeout);
        }
        throw new NetworkError(`Network error: ${error.message}`, error);
      }

      throw new NetworkError('Unknown network error');
    }
  }
}
