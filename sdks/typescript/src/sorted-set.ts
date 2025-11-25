/**
 * Synap TypeScript SDK - Sorted Set Manager
 * 
 * Redis-compatible Sorted Set data structure operations.
 * Sorted Sets are collections of unique members, each associated with a score.
 * Members are ordered by score, enabling range queries, ranking, and leaderboard functionality.
 * 
 * Use cases:
 * - Gaming leaderboards
 * - Priority queues
 * - Rate limiting with timestamps
 * - Time-series data
 * - Auto-complete with relevance scores
 */

import { SynapClient } from './client';

function extractPayload<T>(response: T | { payload: T }): T {
  if (response && typeof response === 'object' && 'payload' in (response as Record<string, unknown>)) {
    return (response as { payload: T }).payload;
  }
  return response as T;
}

/**
 * A member with its score
 */
export interface ScoredMember {
  member: string;
  score: number;
}

/**
 * Statistics for sorted sets
 */
export interface SortedSetStats {
  total_keys: number;
  total_members: number;
  avg_members_per_key: number;
  memory_bytes: number;
}

/**
 * Sorted Set operations manager
 */
export class SortedSetManager {
  constructor(private client: SynapClient) {}

  /**
   * Add member with score to sorted set (ZADD)
   * 
   * @example
   * ```typescript
   * await sortedSet.add('leaderboard', 'player1', 100);
   * ```
   */
  async add(key: string, member: string, score: number): Promise<boolean> {
    const response = await this.client.sendCommand<{ added?: number } | { payload: { added?: number } }>(
      'sortedset.zadd',
      {
        key,
        member,
        score,
      }
    );
    const payload = extractPayload(response);
    return (payload?.added ?? 0) > 0;
  }

  /**
   * Remove members from sorted set (ZREM)
   */
  async rem(key: string, ...members: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ removed?: number } | { payload: { removed?: number } }>(
      'sortedset.zrem',
      {
        key,
        members,
      }
    );
    const payload = extractPayload(response);
    return payload?.removed ?? 0;
  }

  /**
   * Get score of a member (ZSCORE)
   */
  async score(key: string, member: string): Promise<number | null> {
    const response = await this.client.sendCommand<{ score?: number } | { payload: { score?: number } }>(
      'sortedset.zscore',
      {
        key,
        member,
      }
    );
    const payload = extractPayload(response);
    return payload?.score ?? null;
  }

  /**
   * Get cardinality (number of members) (ZCARD)
   */
  async card(key: string): Promise<number> {
    const response = await this.client.sendCommand<{ count?: number } | { payload: { count?: number } }>(
      'sortedset.zcard',
      {
        key,
      }
    );
    const payload = extractPayload(response);
    return payload?.count ?? 0;
  }

  /**
   * Increment score of member (ZINCRBY)
   */
  async incrBy(key: string, member: string, increment: number): Promise<number> {
    const response = await this.client.sendCommand<{ score?: number } | { payload: { score?: number } }>(
      'sortedset.zincrby',
      {
        key,
        member,
        increment,
      }
    );
    const payload = extractPayload(response);
    return payload?.score ?? 0;
  }

  /**
   * Get range by rank (0-based index) (ZRANGE)
   * 
   * @param start - Start index (supports negative indices)
   * @param stop - Stop index (supports negative indices, -1 = last element)
   * @param withScores - Include scores in result
   * 
   * @example
   * ```typescript
   * // Get top 10 from leaderboard
   * const top10 = await sortedSet.range('leaderboard', 0, 9, true);
   * top10.forEach(m => console.log(`${m.member}: ${m.score}`));
   * ```
   */
  async range(
    key: string,
    start: number = 0,
    stop: number = -1,
    withScores: boolean = false
  ): Promise<ScoredMember[]> {
    const response = await this.client.sendCommand<{ members?: ScoredMember[] } | { payload: { members?: ScoredMember[] } }>(
      'sortedset.zrange',
      {
        key,
        start,
        stop,
        withscores: withScores,
      }
    );
    const payload = extractPayload(response);
    return payload?.members ?? [];
  }

  /**
   * Get reverse range by rank (highest to lowest) (ZREVRANGE)
   */
  async revRange(
    key: string,
    start: number = 0,
    stop: number = -1,
    withScores: boolean = false
  ): Promise<ScoredMember[]> {
    const response = await this.client.sendCommand<{ members?: ScoredMember[] } | { payload: { members?: ScoredMember[] } }>(
      'sortedset.zrevrange',
      {
        key,
        start,
        stop,
        withscores: withScores,
      }
    );
    const payload = extractPayload(response);
    return payload?.members ?? [];
  }

  /**
   * Get rank of member (0-based, lowest score = rank 0) (ZRANK)
   */
  async rank(key: string, member: string): Promise<number | null> {
    const response = await this.client.sendCommand<{ rank?: number } | { payload: { rank?: number } }>(
      'sortedset.zrank',
      {
        key,
        member,
      }
    );
    const payload = extractPayload(response);
    return payload?.rank ?? null;
  }

  /**
   * Get reverse rank of member (0-based, highest score = rank 0) (ZREVRANK)
   */
  async revRank(key: string, member: string): Promise<number | null> {
    const response = await this.client.sendCommand<{ rank?: number } | { payload: { rank?: number } }>(
      'sortedset.zrevrank',
      {
        key,
        member,
      }
    );
    const payload = extractPayload(response);
    return payload?.rank ?? null;
  }

  /**
   * Count members with scores in range (ZCOUNT)
   */
  async count(key: string, min: number, max: number): Promise<number> {
    const response = await this.client.sendCommand<{ count?: number } | { payload: { count?: number } }>(
      'sortedset.zcount',
      {
        key,
        min,
        max,
      }
    );
    const payload = extractPayload(response);
    return payload?.count ?? 0;
  }

  /**
   * Get range by score (ZRANGEBYSCORE)
   */
  async rangeByScore(
    key: string,
    min: number,
    max: number,
    withScores: boolean = false
  ): Promise<ScoredMember[]> {
    const response = await this.client.sendCommand<{ members?: ScoredMember[] } | { payload: { members?: ScoredMember[] } }>(
      'sortedset.zrangebyscore',
      {
        key,
        min,
        max,
        withscores: withScores,
      }
    );
    const payload = extractPayload(response);
    return payload?.members ?? [];
  }

  /**
   * Pop minimum scored members (ZPOPMIN)
   */
  async popMin(key: string, count: number = 1): Promise<ScoredMember[]> {
    const response = await this.client.sendCommand<{ members?: ScoredMember[] } | { payload: { members?: ScoredMember[] } }>(
      'sortedset.zpopmin',
      {
        key,
        count,
      }
    );
    const payload = extractPayload(response);
    return payload?.members ?? [];
  }

  /**
   * Pop maximum scored members (ZPOPMAX)
   */
  async popMax(key: string, count: number = 1): Promise<ScoredMember[]> {
    const response = await this.client.sendCommand<{ members?: ScoredMember[] } | { payload: { members?: ScoredMember[] } }>(
      'sortedset.zpopmax',
      {
        key,
        count,
      }
    );
    const payload = extractPayload(response);
    return payload?.members ?? [];
  }

  /**
   * Remove members by rank range (ZREMRANGEBYRANK)
   */
  async remRangeByRank(key: string, start: number, stop: number): Promise<number> {
    const response = await this.client.sendCommand<{ removed?: number } | { payload: { removed?: number } }>(
      'sortedset.zremrangebyrank',
      {
        key,
        start,
        stop,
      }
    );
    const payload = extractPayload(response);
    return payload?.removed ?? 0;
  }

  /**
   * Remove members by score range (ZREMRANGEBYSCORE)
   */
  async remRangeByScore(key: string, min: number, max: number): Promise<number> {
    const response = await this.client.sendCommand<{ removed?: number } | { payload: { removed?: number } }>(
      'sortedset.zremrangebyscore',
      {
        key,
        min,
        max,
      }
    );
    const payload = extractPayload(response);
    return payload?.removed ?? 0;
  }

  /**
   * Compute intersection and store in destination (ZINTERSTORE)
   * 
   * @param destination - Destination key
   * @param keys - Source keys to intersect
   * @param weights - Optional weights for each key
   * @param aggregate - Aggregation method: 'sum' | 'min' | 'max'
   * 
   * @example
   * ```typescript
   * // Intersect two leaderboards with weighted scores
   * await sortedSet.interStore('combined', ['board1', 'board2'], [1.0, 2.0], 'sum');
   * ```
   */
  async interStore(
    destination: string,
    keys: string[],
    weights?: number[],
    aggregate: 'sum' | 'min' | 'max' = 'sum'
  ): Promise<number> {
    const response = await this.client.sendCommand<{ count?: number } | { payload: { count?: number } }>(
      'sortedset.zinterstore',
      {
        destination,
        keys,
        weights,
        aggregate,
      }
    );
    const payload = extractPayload(response);
    return payload?.count ?? 0;
  }

  /**
   * Compute union and store in destination (ZUNIONSTORE)
   */
  async unionStore(
    destination: string,
    keys: string[],
    weights?: number[],
    aggregate: 'sum' | 'min' | 'max' = 'sum'
  ): Promise<number> {
    const response = await this.client.sendCommand<{ count?: number } | { payload: { count?: number } }>(
      'sortedset.zunionstore',
      {
        destination,
        keys,
        weights,
        aggregate,
      }
    );
    const payload = extractPayload(response);
    return payload?.count ?? 0;
  }

  /**
   * Compute difference and store in destination (ZDIFFSTORE)
   */
  async diffStore(destination: string, keys: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ count?: number } | { payload: { count?: number } }>(
      'sortedset.zdiffstore',
      {
        destination,
        keys,
      }
    );
    const payload = extractPayload(response);
    return payload?.count ?? 0;
  }

  /**
   * Get statistics
   */
  async stats(): Promise<SortedSetStats> {
    const response = await this.client.sendCommand<SortedSetStats | { payload: SortedSetStats }>(
      'sortedset.stats',
      {}
    );
    const payload = extractPayload(response);
    return payload ?? {
      total_keys: 0,
      total_members: 0,
      avg_members_per_key: 0,
      memory_bytes: 0,
    };
  }
}


