/**
 * Geospatial Manager Unit Tests
 * 
 * Tests for Geospatial operations using mocked HTTP client
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { GeospatialManager } from '../geospatial';
import { SynapClient } from '../client';

describe('GeospatialManager', () => {
  let manager: GeospatialManager;
  let mockClient: SynapClient;

  beforeEach(() => {
    mockClient = {
      sendCommand: vi.fn(),
    } as unknown as SynapClient;
    manager = new GeospatialManager(mockClient);
  });

  describe('geoadd', () => {
    it('should add locations and return count', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        added: 2,
      });

      const result = await manager.geoadd('cities', [
        { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
        { lat: 40.7128, lon: -74.0060, member: 'New York' },
      ]);

      expect(result).toBe(2);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.geoadd', {
        key: 'cities',
        locations: [
          { lat: 37.7749, lon: -122.4194, member: 'San Francisco' },
          { lat: 40.7128, lon: -74.0060, member: 'New York' },
        ],
        nx: false,
        xx: false,
        ch: false,
      });
    });

    it('should validate latitude', async () => {
      await expect(
        manager.geoadd('cities', [{ lat: 91, lon: 0, member: 'invalid' }])
      ).rejects.toThrow('Latitude must be between -90 and 90');
    });

    it('should validate longitude', async () => {
      await expect(
        manager.geoadd('cities', [{ lat: 0, lon: 181, member: 'invalid' }])
      ).rejects.toThrow('Longitude must be between -180 and 180');
    });

    it('should pass nx/xx/ch options', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        added: 1,
      });

      await manager.geoadd(
        'cities',
        [{ lat: 37.7749, lon: -122.4194, member: 'SF' }],
        { nx: true, xx: false, ch: true }
      );

      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.geoadd', {
        key: 'cities',
        locations: [{ lat: 37.7749, lon: -122.4194, member: 'SF' }],
        nx: true,
        xx: false,
        ch: true,
      });
    });
  });

  describe('geodist', () => {
    it('should calculate distance between members', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        distance: 4139.5,
        unit: 'km',
      });

      const result = await manager.geodist('cities', 'San Francisco', 'New York', 'km');

      expect(result).toBe(4139.5);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.geodist', {
        key: 'cities',
        member1: 'San Francisco',
        member2: 'New York',
        unit: 'km',
      });
    });

    it('should return null when member not found', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        distance: null,
        unit: 'm',
      });

      const result = await manager.geodist('cities', 'Unknown1', 'Unknown2');

      expect(result).toBeNull();
    });

    it('should default to meters', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        distance: 1000,
        unit: 'm',
      });

      await manager.geodist('cities', 'SF', 'Oakland');

      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.geodist', {
        key: 'cities',
        member1: 'SF',
        member2: 'Oakland',
        unit: 'm',
      });
    });
  });

  describe('georadius', () => {
    it('should query members within radius', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        results: [
          { member: 'San Francisco', distance: 0 },
          { member: 'Oakland', distance: 20.5 },
        ],
      });

      const result = await manager.georadius('cities', 37.7749, -122.4194, 100, 'km', {
        withDist: true,
      });

      expect(result).toHaveLength(2);
      expect(result[0].member).toBe('San Francisco');
      expect(result[1].distance).toBe(20.5);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.georadius', {
        key: 'cities',
        center_lat: 37.7749,
        center_lon: -122.4194,
        radius: 100,
        unit: 'km',
        with_dist: true,
        with_coord: false,
      });
    });

    it('should validate center coordinates', async () => {
      await expect(
        manager.georadius('cities', 91, 0, 100)
      ).rejects.toThrow('Latitude must be between -90 and 90');

      await expect(
        manager.georadius('cities', 0, 181, 100)
      ).rejects.toThrow('Longitude must be between -180 and 180');
    });

    it('should include coordinates when requested', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        results: [
          {
            member: 'SF',
            distance: 0,
            coord: { lat: 37.7749, lon: -122.4194 },
          },
        ],
      });

      const result = await manager.georadius('cities', 37.7749, -122.4194, 50, 'km', {
        withDist: true,
        withCoord: true,
      });

      expect(result[0].coord).toEqual({ lat: 37.7749, lon: -122.4194 });
    });

    it('should support count and sort options', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        results: [],
      });

      await manager.georadius('cities', 37.7749, -122.4194, 100, 'km', {
        count: 10,
        sort: 'ASC',
      });

      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.georadius', {
        key: 'cities',
        center_lat: 37.7749,
        center_lon: -122.4194,
        radius: 100,
        unit: 'km',
        with_dist: false,
        with_coord: false,
        count: 10,
        sort: 'ASC',
      });
    });
  });

  describe('georadiusbymember', () => {
    it('should query members within radius of member', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        results: [
          { member: 'San Francisco', distance: 0 },
          { member: 'Oakland', distance: 20.5 },
        ],
      });

      const result = await manager.georadiusbymember('cities', 'San Francisco', 50, 'km', {
        withDist: true,
      });

      expect(result).toHaveLength(2);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.georadiusbymember', {
        key: 'cities',
        member: 'San Francisco',
        radius: 50,
        unit: 'km',
        with_dist: true,
        with_coord: false,
      });
    });
  });

  describe('geopos', () => {
    it('should get coordinates of members', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        coordinates: [
          { lat: 37.7749, lon: -122.4194 },
          { lat: 40.7128, lon: -74.0060 },
          null,
        ],
      });

      const result = await manager.geopos('cities', ['San Francisco', 'New York', 'Unknown']);

      expect(result).toHaveLength(3);
      expect(result[0]).toEqual({ lat: 37.7749, lon: -122.4194 });
      expect(result[1]).toEqual({ lat: 40.7128, lon: -74.0060 });
      expect(result[2]).toBeNull();
    });
  });

  describe('geohash', () => {
    it('should get geohash strings for members', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'cities',
        geohashes: ['9q8yyb1v5b0', 'dr5regy3vg0', null],
      });

      const result = await manager.geohash('cities', ['San Francisco', 'New York', 'Unknown']);

      expect(result).toHaveLength(3);
      expect(result[0]).toBe('9q8yyb1v5b0');
      expect(result[1]).toBe('dr5regy3vg0');
      expect(result[2]).toBeNull();
    });
  });

  describe('stats', () => {
    it('should retrieve geospatial statistics', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        total_keys: 5,
        total_locations: 25,
        geoadd_count: 25,
        geodist_count: 10,
        georadius_count: 15,
        geopos_count: 8,
        geohash_count: 5,
      });

      const result = await manager.stats();

      expect(result.total_keys).toBe(5);
      expect(result.total_locations).toBe(25);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('geospatial.stats', {});
    });
  });
});

