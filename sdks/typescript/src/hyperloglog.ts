import { SynapClient } from './client';
import type { CommandOptions, HyperLogLogStats } from './types';

function encodeElement(element: string | Uint8Array | number[]): number[] {
  if (typeof element === 'string') {
    return Array.from(new TextEncoder().encode(element));
  }

  if (element instanceof Uint8Array) {
    return Array.from(element);
  }

  if (Array.isArray(element)) {
    return element;
  }

  throw new TypeError('Unsupported element type for HyperLogLog. Use string, Uint8Array, or number[].');
}

interface PfAddResponse {
  added?: number;
}

interface PfCountResponse {
  count?: number;
}

interface PfMergeResponse {
  count?: number;
}

/**
 * HyperLogLog manager for probabilistic cardinality estimation.
 */
export class HyperLogLogManager {
  constructor(private readonly client: SynapClient) {}

  private buildPayload(
    options: CommandOptions | undefined,
    extra: Record<string, unknown>
  ): Record<string, unknown> {
    const payload: Record<string, unknown> = { ...extra };

    if (options?.clientId) {
      payload.client_id = options.clientId;
    }

    return payload;
  }

  /**
   * Add elements to a HyperLogLog structure (PFADD)
   */
  async pfadd(
    key: string,
    elements: Array<string | Uint8Array | number[]>,
    options?: CommandOptions
  ): Promise<number> {
    if (!elements.length) {
      return 0;
    }

    const encoded = elements.map(encodeElement);
    const response = await this.client.sendCommand<PfAddResponse>(
      'hyperloglog.pfadd',
      this.buildPayload(options, {
        key,
        elements: encoded,
      })
    );

    return response.added ?? 0;
  }

  /**
   * Estimate cardinality of a HyperLogLog structure (PFCOUNT)
   */
  async pfcount(key: string, options?: CommandOptions): Promise<number> {
    const response = await this.client.sendCommand<PfCountResponse>(
      'hyperloglog.pfcount',
      this.buildPayload(options, { key })
    );

    return response.count ?? 0;
  }

  /**
   * Merge multiple HyperLogLog structures (PFMERGE)
   */
  async pfmerge(
    destination: string,
    sources: string[],
    options?: CommandOptions
  ): Promise<number> {
    const response = await this.client.sendCommand<PfMergeResponse>(
      'hyperloglog.pfmerge',
      this.buildPayload(options, {
        destination,
        sources,
      })
    );

    return response.count ?? 0;
  }

  /**
   * Retrieve HyperLogLog statistics
   */
  async stats(options?: CommandOptions): Promise<HyperLogLogStats> {
    const response = await this.client.sendCommand<HyperLogLogStats>(
      'hyperloglog.stats',
      this.buildPayload(options, {})
    );

    return response;
  }
}
