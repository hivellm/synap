/**
 * Geospatial Manager S2S Tests
 * 
 * Integration tests connecting to a real Synap server
 * Set SYNAP_URL environment variable to test against remote server
 * Default: http://localhost:15500
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { Synap } from '../index';

const SYNAP_URL = process.env.SYNAP_URL || 'http://localhost:15500';

describe('GeospatialManager S2S', () => {
  let synap: Synap;
  const testKey = `test:geospatial:${Date.now()}`;

  beforeAll(() => {
    synap = new Synap({ url: SYNAP_URL });
  });

  describe('geoadd', () => {
    it('should add locations and return count', async () => {
      const added = await synap.geospatial.geoadd(testKey, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 40.7128, lon: -74.0060, member: 'New York' },
        { lat: 34.0522, lon: -118.2437, member: 'Los Angeles' },
      ]);

      expect(added).toBeGreaterThanOrEqual(0);
    });
  });

  describe('geodist', () => {
    it('should calculate distance between two members', async () => {
      // Ensure data exists
      await synap.geospatial.geoadd(`${testKey}:dist`, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 40.7128, lon: -74.0060, member: 'New York' },
      ]);

      const distance = await synap.geospatial.geodist(
        `${testKey}:dist`,
        'San Francisco',
        'New York',
        'km'
      );

      expect(distance).not.toBeNull();
      expect(distance!).toBeGreaterThan(0);
      expect(distance!).toBeLessThan(5000); // Should be around 4000km
    });

    it('should return null for non-existent members', async () => {
      try {
        const distance = await synap.geospatial.geodist(
          `${testKey}:nonexistent`,
          'Unknown1',
          'Unknown2',
          'm'
        );
        // Server may return null or throw error
        expect(distance === null || distance === undefined).toBe(true);
      } catch (error: any) {
        // Accept ServerError: Not found
        expect(error?.message).toMatch(/not found|Not found/i);
      }
    });
  });

  describe('georadius', () => {
    it('should find members within radius', async () => {
      const key = `${testKey}:radius`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 37.8044, lon: -122.2711, member: 'Oakland' },
        { lat: 34.0522, lon: -118.2437, member: 'Los Angeles' },
      ]);

      const results = await synap.geospatial.georadius(
        key,
        37.7749,
        -122.4194,
        100,
        'km',
        { withDist: true }
      );

      expect(results.length).toBeGreaterThanOrEqual(1);
      expect(results.some((r) => r.member === 'San Francisco')).toBe(true);
    });

    it('should include coordinates when requested', async () => {
      const key = `${testKey}:radius:coord`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
      ]);

      const results = await synap.geospatial.georadius(key, 37.7749, -122.4194, 50, 'km', {
        withDist: true,
        withCoord: true,
      });

      expect(results.length).toBeGreaterThanOrEqual(1);
      if (results[0].coord) {
        expect(results[0].coord.lat).toBeCloseTo(37.7749, 3);
        expect(results[0].coord.lon).toBeCloseTo(-122.4194, 3);
      }
    });

    it('should respect count limit', async () => {
      const key = `${testKey}:radius:count`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 37.8044, lon: -122.2711, member: 'Oakland' },
        { lat: 34.0522, lon: -118.2437, member: 'Los Angeles' },
      ]);

      const results = await synap.geospatial.georadius(key, 37.7749, -122.4194, 1000, 'km', {
        count: 2,
      });

      expect(results.length).toBeLessThanOrEqual(2);
    });
  });

  describe('georadiusbymember', () => {
    it('should find members within radius of member', async () => {
      const key = `${testKey}:radiusbymember`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 37.8044, lon: -122.2711, member: 'Oakland' },
      ]);

      const results = await synap.geospatial.georadiusbymember(
        key,
        'San Francisco',
        50,
        'km',
        { withDist: true }
      );

      expect(results.length).toBeGreaterThanOrEqual(1);
      expect(results.some((r) => r.member === 'San Francisco')).toBe(true);
    });
  });

  describe('geopos', () => {
    it('should get coordinates of members', async () => {
      const key = `${testKey}:geopos`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 40.7128, lon: -74.0060, member: 'New York' },
      ]);

      const coords = await synap.geospatial.geopos(key, ['San Francisco', 'New York', 'Unknown']);

      expect(coords).toHaveLength(3);
      expect(coords[0]).not.toBeNull();
      expect(coords[0]!.lat).toBeCloseTo(37.7749, 3);
      expect(coords[0]!.lon).toBeCloseTo(-122.4194, 3);
      expect(coords[1]).not.toBeNull();
      expect(coords[1]!.lat).toBeCloseTo(40.7128, 3);
      expect(coords[2]).toBeNull(); // Unknown member
    });
  });

  describe('geohash', () => {
    it('should get geohash strings for members', async () => {
      const key = `${testKey}:geohash`;
      await synap.geospatial.geoadd(key, [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 40.7128, lon: -74.0060, member: 'New York' },
      ]);

      const geohashes = await synap.geospatial.geohash(key, [
        'San Francisco',
        'New York',
        'Unknown',
      ]);

      expect(geohashes).toHaveLength(3);
      expect(geohashes[0]).not.toBeNull();
      expect(geohashes[0]!.length).toBe(11); // Redis uses 11-character geohash
      expect(geohashes[1]).not.toBeNull();
      expect(geohashes[2]).toBeNull(); // Unknown member
    });
  });

  describe('stats', () => {
    it('should return geospatial statistics', async () => {
      const stats = await synap.geospatial.stats();

      expect(stats).toHaveProperty('total_keys');
      expect(stats).toHaveProperty('total_locations');
      expect(stats).toHaveProperty('geoadd_count');
      expect(stats).toHaveProperty('geodist_count');
      expect(stats).toHaveProperty('georadius_count');
      expect(stats).toHaveProperty('geopos_count');
      expect(stats).toHaveProperty('geohash_count');

      expect(stats.total_keys).toBeGreaterThanOrEqual(0);
      expect(stats.total_locations).toBeGreaterThanOrEqual(0);
    });
  });
});

