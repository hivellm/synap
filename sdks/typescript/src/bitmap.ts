/**
 * Synap TypeScript SDK - Bitmap Manager
 * 
 * Redis-compatible Bitmap operations for bit-level manipulation
 */

import { SynapClient } from './client';
import type { CommandOptions } from './types';

/**
 * Bitmap operation types (AND, OR, XOR, NOT)
 */
export type BitmapOperation = 'AND' | 'OR' | 'XOR' | 'NOT';

/**
 * Bitmap statistics
 */
export interface BitmapStats {
  total_bitmaps: number;
  total_bits: number;
  setbit_count: number;
  getbit_count: number;
  bitcount_count: number;
  bitop_count: number;
  bitpos_count: number;
  bitfield_count: number;
}

interface SetBitResponse {
  key: string;
  old_value: number;
}

interface GetBitResponse {
  key: string;
  offset: number;
  value: number;
}

interface BitCountResponse {
  key: string;
  count: number;
}

interface BitPosResponse {
  key: string;
  position: number | null;
}

interface BitOpResponse {
  destination: string;
  length: number;
}

/**
 * Bitfield operation type
 */
export type BitfieldOperationType = 'GET' | 'SET' | 'INCRBY';

/**
 * Bitfield overflow behavior
 */
export type BitfieldOverflow = 'WRAP' | 'SAT' | 'FAIL';

/**
 * Bitfield operation specification
 */
export interface BitfieldOperation {
  operation: BitfieldOperationType;
  offset: number;
  width: number;
  signed?: boolean;
  value?: number; // Required for SET
  increment?: number; // Required for INCRBY
  overflow?: BitfieldOverflow; // For INCRBY, default: WRAP
}

interface BitfieldResponse {
  key: string;
  results: number[];
}

/**
 * Bitmap operations manager
 */
export class BitmapManager {
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
   * Set bit at offset to value (SETBIT)
   * @param key Bitmap key
   * @param offset Bit offset (0-based)
   * @param value Bit value (0 or 1)
   * @param options Optional command options
   * @returns Previous bit value (0 or 1)
   */
  async setbit(
    key: string,
    offset: number,
    value: 0 | 1,
    options?: CommandOptions
  ): Promise<number> {
    if (value !== 0 && value !== 1) {
      throw new TypeError('Bitmap value must be 0 or 1');
    }

    const response = await this.client.sendCommand<SetBitResponse>(
      'bitmap.setbit',
      this.buildPayload(options, {
        key,
        offset,
        value,
      })
    );

    return response.old_value ?? 0;
  }

  /**
   * Get bit at offset (GETBIT)
   * @param key Bitmap key
   * @param offset Bit offset (0-based)
   * @param options Optional command options
   * @returns Bit value (0 or 1)
   */
  async getbit(key: string, offset: number, options?: CommandOptions): Promise<number> {
    const response = await this.client.sendCommand<GetBitResponse>(
      'bitmap.getbit',
      this.buildPayload(options, {
        key,
        offset,
      })
    );

    return response.value ?? 0;
  }

  /**
   * Count set bits in bitmap (BITCOUNT)
   * @param key Bitmap key
   * @param start Optional start offset (inclusive, default: 0)
   * @param end Optional end offset (inclusive, default: end of bitmap)
   * @param options Optional command options
   * @returns Number of set bits
   */
  async bitcount(
    key: string,
    start?: number,
    end?: number,
    options?: CommandOptions
  ): Promise<number> {
    const payload: Record<string, unknown> = { key };
    if (start !== undefined) {
      payload.start = start;
    }
    if (end !== undefined) {
      payload.end = end;
    }

    const response = await this.client.sendCommand<BitCountResponse>(
      'bitmap.bitcount',
      this.buildPayload(options, payload)
    );

    return response.count ?? 0;
  }

  /**
   * Find first bit set to value (BITPOS)
   * @param key Bitmap key
   * @param value Bit value to search for (0 or 1)
   * @param start Optional start offset (inclusive, default: 0)
   * @param end Optional end offset (inclusive, default: end of bitmap)
   * @param options Optional command options
   * @returns Position of first matching bit, or null if not found
   */
  async bitpos(
    key: string,
    value: 0 | 1,
    start?: number,
    end?: number,
    options?: CommandOptions
  ): Promise<number | null> {
    if (value !== 0 && value !== 1) {
      throw new TypeError('Bitmap value must be 0 or 1');
    }

    const payload: Record<string, unknown> = { key, value };
    if (start !== undefined) {
      payload.start = start;
    }
    if (end !== undefined) {
      payload.end = end;
    }

    const response = await this.client.sendCommand<BitPosResponse>(
      'bitmap.bitpos',
      this.buildPayload(options, payload)
    );

    return response.position ?? null;
  }

  /**
   * Perform bitwise operation on multiple bitmaps (BITOP)
   * @param operation Bitwise operation (AND, OR, XOR, NOT)
   * @param destination Destination key for result
   * @param sourceKeys Source bitmap keys (NOT requires exactly 1 source)
   * @param options Optional command options
   * @returns Length of resulting bitmap in bits
   */
  async bitop(
    operation: BitmapOperation,
    destination: string,
    sourceKeys: string[],
    options?: CommandOptions
  ): Promise<number> {
    if (operation === 'NOT' && sourceKeys.length !== 1) {
      throw new Error('NOT operation requires exactly one source key');
    }

    if (operation !== 'NOT' && sourceKeys.length < 1) {
      throw new Error('BITOP requires at least one source key');
    }

    const response = await this.client.sendCommand<BitOpResponse>(
      'bitmap.bitop',
      this.buildPayload(options, {
        destination,
        operation,
        source_keys: sourceKeys,
      })
    );

    return response.length ?? 0;
  }

  /**
   * Execute bitfield operations (BITFIELD)
   * @param key Bitmap key
   * @param operations List of bitfield operations
   * @param options Optional command options
   * @returns List of result values (one per operation)
   */
  async bitfield(
    key: string,
    operations: BitfieldOperation[],
    options?: CommandOptions
  ): Promise<number[]> {
    const response = await this.client.sendCommand<BitfieldResponse>(
      'bitmap.bitfield',
      this.buildPayload(options, {
        key,
        operations,
      })
    );

    return response.results ?? [];
  }

  /**
   * Retrieve bitmap statistics
   * @param options Optional command options
   * @returns Bitmap statistics
   */
  async stats(options?: CommandOptions): Promise<BitmapStats> {
    const response = await this.client.sendCommand<BitmapStats>(
      'bitmap.stats',
      this.buildPayload(options, {})
    );

    return response;
  }
}

