/**
 * Sorted Set Manager Unit Tests
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { SortedSetManager } from '../sorted-set';
import { SynapClient } from '../client';

describe('SortedSetManager', () => {
  let client: SynapClient;
  let sortedSet: SortedSetManager;

  beforeEach(() => {
    client = new SynapClient({ url: 'http://localhost:15500' });
    sortedSet = new SortedSetManager(client);

    // Mock sendCommand
    vi.spyOn(client, 'sendCommand').mockImplementation(async (command, payload) => {
      // Return appropriate mock responses based on command
      if (command === 'sortedset.zadd') {
        return { payload: { added: 1, key: payload.key } };
      }
      if (command === 'sortedset.zscore') {
        return { payload: { score: 100.0, key: payload.key, member: payload.member } };
      }
      if (command === 'sortedset.zcard') {
        return { payload: { count: 3, key: payload.key } };
      }
      if (command === 'sortedset.zrange') {
        return {
          payload: {
            members: [
              { member: 'alice', score: 100.0 },
              { member: 'bob', score: 200.0 },
            ],
            key: payload.key,
          },
        };
      }
      if (command === 'sortedset.zrank') {
        return { payload: { rank: 0, key: payload.key, member: payload.member } };
      }
      if (command === 'sortedset.zpopmin') {
        return {
          payload: {
            members: [{ member: 'alice', score: 100.0 }],
            count: 1,
            key: payload.key,
          },
        };
      }
      if (command === 'sortedset.zinterstore') {
        return { payload: { count: 2, destination: payload.destination } };
      }
      if (command === 'sortedset.stats') {
        return {
          payload: {
            total_keys: 5,
            total_members: 50,
            avg_members_per_key: 10.0,
            memory_bytes: 4096,
          },
        };
      }
      return { payload: {} };
    });
  });

  describe('basic operations', () => {
    it('should add member with score', async () => {
      const added = await sortedSet.add('leaderboard', 'player1', 100.0);
      expect(added).toBe(true);
      expect(client.sendCommand).toHaveBeenCalledWith('sortedset.zadd', {
        key: 'leaderboard',
        member: 'player1',
        score: 100.0,
      });
    });

    it('should get score of member', async () => {
      const score = await sortedSet.score('leaderboard', 'player1');
      expect(score).toBe(100.0);
    });

    it('should get cardinality', async () => {
      const count = await sortedSet.card('leaderboard');
      expect(count).toBe(3);
    });

    it('should increment score', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { score: 150.0, key: 'leaderboard' },
      });
      const newScore = await sortedSet.incrBy('leaderboard', 'player1', 50.0);
      expect(newScore).toBe(150.0);
    });
  });

  describe('range operations', () => {
    it('should get range by rank', async () => {
      const members = await sortedSet.range('leaderboard', 0, 1, true);
      expect(members).toHaveLength(2);
      expect(members[0].member).toBe('alice');
      expect(members[0].score).toBe(100.0);
    });

    it('should get reverse range', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: {
          members: [
            { member: 'bob', score: 200.0 },
            { member: 'alice', score: 100.0 },
          ],
        },
      });
      const members = await sortedSet.revRange('leaderboard', 0, 1, true);
      expect(members).toHaveLength(2);
      expect(members[0].member).toBe('bob');
    });

    it('should get range by score', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: {
          members: [{ member: 'alice', score: 100.0 }],
        },
      });
      const members = await sortedSet.rangeByScore('leaderboard', 50.0, 150.0, true);
      expect(members).toHaveLength(1);
      expect(members[0].member).toBe('alice');
    });

    it('should count members in range', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { count: 2 },
      });
      const count = await sortedSet.count('leaderboard', 50.0, 150.0);
      expect(count).toBe(2);
    });
  });

  describe('ranking operations', () => {
    it('should get rank of member', async () => {
      const rank = await sortedSet.rank('leaderboard', 'player1');
      expect(rank).toBe(0);
    });

    it('should get reverse rank', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { rank: 2 },
      });
      const rank = await sortedSet.revRank('leaderboard', 'player1');
      expect(rank).toBe(2);
    });
  });

  describe('pop operations', () => {
    it('should pop minimum scored members', async () => {
      const members = await sortedSet.popMin('tasks', 1);
      expect(members).toHaveLength(1);
      expect(members[0].member).toBe('alice');
      expect(members[0].score).toBe(100.0);
    });

    it('should pop maximum scored members', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: {
          members: [{ member: 'charlie', score: 300.0 }],
        },
      });
      const members = await sortedSet.popMax('tasks', 1);
      expect(members).toHaveLength(1);
      expect(members[0].member).toBe('charlie');
    });
  });

  describe('remove range operations', () => {
    it('should remove range by rank', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { removed: 3 },
      });
      const removed = await sortedSet.remRangeByRank('scores', 0, 2);
      expect(removed).toBe(3);
    });

    it('should remove range by score', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { removed: 5 },
      });
      const removed = await sortedSet.remRangeByScore('scores', 0.0, 50.0);
      expect(removed).toBe(5);
    });
  });

  describe('set operations', () => {
    it('should compute intersection and store', async () => {
      const count = await sortedSet.interStore('combined', ['zset1', 'zset2']);
      expect(count).toBe(2);
      expect(client.sendCommand).toHaveBeenCalledWith('sortedset.zinterstore', {
        destination: 'combined',
        keys: ['zset1', 'zset2'],
        weights: undefined,
        aggregate: 'sum',
      });
    });

    it('should compute union with weights', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { count: 4, destination: 'union' },
      });
      const count = await sortedSet.unionStore('union', ['zset1', 'zset2'], [1.0, 2.0], 'max');
      expect(count).toBe(4);
    });

    it('should compute difference', async () => {
      vi.spyOn(client, 'sendCommand').mockResolvedValueOnce({
        payload: { count: 1, destination: 'diff' },
      });
      const count = await sortedSet.diffStore('diff', ['zset1', 'zset2']);
      expect(count).toBe(1);
    });
  });

  describe('statistics', () => {
    it('should get sorted set statistics', async () => {
      const stats = await sortedSet.stats();
      expect(stats.total_keys).toBe(5);
      expect(stats.total_members).toBe(50);
      expect(stats.avg_members_per_key).toBe(10.0);
      expect(stats.memory_bytes).toBe(4096);
    });
  });
});


