/**
 * Synap TypeScript SDK - Base Client
 * 
 * Core HTTP client for StreamableHTTP protocol communication.
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  SynapRequest,
  SynapResponse,
  SynapClientOptions,
  AuthOptions,
} from './types';
import { NetworkError, ServerError, TimeoutError } from './types';

/**
 * Base HTTP client for Synap server communication
 */
export class SynapClient {
  private readonly baseUrl: string;
  private readonly timeout: number;
  private readonly debug: boolean;
  private readonly auth?: AuthOptions;

  constructor(options: SynapClientOptions = {}) {
    this.baseUrl = options.url || 'http://localhost:15500';
    this.timeout = options.timeout || 30000;
    this.debug = options.debug || false;
    this.auth = options.auth;
  }

  /**
   * Send a command to the Synap server using StreamableHTTP protocol
   */
  async sendCommand<T = any>(
    command: string,
    payload: Record<string, any> = {}
  ): Promise<T> {
    const request: SynapRequest = {
      command,
      request_id: uuidv4(),
      payload,
    };

    if (this.debug) {
      console.log('[Synap] Request:', JSON.stringify(request, null, 2));
    }

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), this.timeout);

      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'Accept-Encoding': 'gzip',
      };

      // Add authentication headers
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
          request.request_id
        );
      }

      const data = await response.json() as SynapResponse<T>;

      if (this.debug) {
        console.log('[Synap] Response:', JSON.stringify(data, null, 2));
      }

      // Check StreamableHTTP envelope
      if (!data.success) {
        throw new ServerError(
          data.error || 'Unknown server error',
          undefined,
          data.request_id
        );
      }

      return data.payload as T;
    } catch (error) {
      if (error instanceof ServerError) {
        throw error;
      }

      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          throw new TimeoutError(
            `Request timed out after ${this.timeout}ms`,
            this.timeout
          );
        }

        throw new NetworkError(
          `Network error: ${error.message}`,
          error
        );
      }

      throw new NetworkError('Unknown network error');
    }
  }

  /**
   * Ping the server to check connectivity
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
   * Get server health status
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
   * Close the client (cleanup)
   */
  close(): void {
    // Future: cleanup any persistent connections
  }
}

