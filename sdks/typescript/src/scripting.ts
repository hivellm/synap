import { SynapClient } from './client';

export interface ScriptEvalOptions {
  /** Keys accessible within the Lua script (KEYS array) */
  keys?: string[];
  /** Arguments passed to the Lua script (ARGV array) */
  args?: unknown[];
  /** Optional execution timeout (in milliseconds) */
  timeoutMs?: number;
}

export interface ScriptEvalResponse<T = unknown> {
  /** Script execution result */
  result: T;
  /** SHA1 hash of the script (useful for caching/EVALSHA) */
  sha1: string;
}

export interface ScriptExistsResponse {
  /** Boolean flags indicating whether each requested hash exists */
  exists: boolean[];
}

export interface ScriptFlushResponse {
  /** Number of cached scripts cleared */
  cleared: number;
}

export interface ScriptKillResponse {
  /** Indicates whether a running script was terminated */
  terminated: boolean;
}

/**
 * Lua scripting manager.
 * Provides helpers for `EVAL`, `EVALSHA`, and SCRIPT subcommands.
 */
export class ScriptManager {
  constructor(private readonly client: SynapClient) {}

  /**
   * Execute a Lua script using `EVAL`
   */
  async eval<T = unknown>(script: string, options: ScriptEvalOptions = {}): Promise<ScriptEvalResponse<T>> {
    const payload: Record<string, unknown> = {
      script,
      keys: options.keys ?? [],
      args: options.args ?? [],
    };

    if (options.timeoutMs !== undefined) {
      payload.timeout_ms = options.timeoutMs;
    }

    const response = await this.client.sendCommand<{ result: T; sha1: string }>('script.eval', payload);

    return {
      result: response.result,
      sha1: response.sha1,
    };
  }

  /**
   * Execute a cached script using `EVALSHA`
   */
  async evalsha<T = unknown>(sha1: string, options: ScriptEvalOptions = {}): Promise<ScriptEvalResponse<T>> {
    const payload: Record<string, unknown> = {
      sha1,
      keys: options.keys ?? [],
      args: options.args ?? [],
    };

    if (options.timeoutMs !== undefined) {
      payload.timeout_ms = options.timeoutMs;
    }

    const response = await this.client.sendCommand<{ result: T; sha1: string }>(
      'script.evalsha',
      payload
    );

    return {
      result: response.result,
      sha1: response.sha1,
    };
  }

  /**
   * Load a script into the server cache and return its SHA1 hash
   */
  async load(script: string): Promise<string> {
    const response = await this.client.sendCommand<{ sha1: string }>('script.load', {
      script,
    });

    return response.sha1;
  }

  /**
   * Check whether script hashes exist in the cache
   */
  async exists(hashes: string[]): Promise<boolean[]> {
    const response = await this.client.sendCommand<ScriptExistsResponse>('script.exists', {
      hashes,
    });

    return response.exists;
  }

  /**
   * Flush all cached scripts
   */
  async flush(): Promise<number> {
    const response = await this.client.sendCommand<ScriptFlushResponse>('script.flush', {});
    return response.cleared;
  }

  /**
   * Kill the currently executing script (if any)
   */
  async kill(): Promise<boolean> {
    const response = await this.client.sendCommand<ScriptKillResponse>('script.kill', {});
    return response.terminated;
  }
}
