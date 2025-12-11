/**
 * Synap TypeScript SDK - Geospatial Manager
 * 
 * Redis-compatible Geospatial operations for location-based queries
 */

import { SynapClient } from './client';
import type { CommandOptions } from './types';

/**
 * Distance unit types
 */
export type DistanceUnit = 'm' | 'km' | 'mi' | 'ft';

/**
 * Geospatial statistics
 */
export interface GeospatialStats {
  total_keys: number;
  total_locations: number;
  geoadd_count: number;
  geodist_count: number;
  georadius_count: number;
  geopos_count: number;
  geohash_count: number;
}

/**
 * Location coordinate
 */
export interface Location {
  lat: number;
  lon: number;
  member: string;
}

/**
 * Geospatial coordinate
 */
export interface Coordinate {
  lat: number;
  lon: number;
}

/**
 * Georadius result item
 */
export interface GeoradiusResult {
  member: string;
  distance?: number;
  coord?: Coordinate;
}

interface GeoaddResponse {
  key: string;
  added: number;
}

interface GeodistResponse {
  key: string;
  distance: number | null;
  unit: string;
}

interface GeoradiusResponse {
  key: string;
  results: GeoradiusResult[];
}

interface GeoposResponse {
  key: string;
  coordinates: (Coordinate | null)[];
}

interface GeohashResponse {
  key: string;
  geohashes: (string | null)[];
}

/**
 * Geospatial operations manager
 */
export class GeospatialManager {
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
   * Add geospatial locations (GEOADD)
   * @param key Geospatial key
   * @param locations Array of locations (lat, lon, member)
   * @param options Optional options (nx, xx, ch)
   * @returns Number of elements added
   */
  async geoadd(
    key: string,
    locations: Location[],
    options?: CommandOptions & { nx?: boolean; xx?: boolean; ch?: boolean }
  ): Promise<number> {
    // Validate coordinates
    for (const loc of locations) {
      if (loc.lat < -90 || loc.lat > 90) {
        throw new TypeError(`Latitude must be between -90 and 90, got: ${loc.lat}`);
      }
      if (loc.lon < -180 || loc.lon > 180) {
        throw new TypeError(`Longitude must be between -180 and 180, got: ${loc.lon}`);
      }
    }

    const response = await this.client.sendCommand<GeoaddResponse>(
      'geospatial.geoadd',
      this.buildPayload(options, {
        key,
        locations,
        nx: options?.nx ?? false,
        xx: options?.xx ?? false,
        ch: options?.ch ?? false,
      })
    );

    return response.added;
  }

  /**
   * Calculate distance between two members (GEODIST)
   * @param key Geospatial key
   * @param member1 First member
   * @param member2 Second member
   * @param unit Distance unit (default: 'm')
   * @param options Optional command options
   * @returns Distance in specified unit, or null if either member doesn't exist
   */
  async geodist(
    key: string,
    member1: string,
    member2: string,
    unit: DistanceUnit = 'm',
    options?: CommandOptions
  ): Promise<number | null> {
    const response = await this.client.sendCommand<GeodistResponse>(
      'geospatial.geodist',
      this.buildPayload(options, {
        key,
        member1,
        member2,
        unit,
      })
    );

    return response.distance;
  }

  /**
   * Query members within radius (GEORADIUS)
   * @param key Geospatial key
   * @param centerLat Center latitude
   * @param centerLon Center longitude
   * @param radius Radius
   * @param unit Distance unit (default: 'm')
   * @param options Query options (withDist, withCoord, count, sort)
   * @param commandOptions Optional command options
   * @returns Array of matching members with optional distance and coordinates
   */
  async georadius(
    key: string,
    centerLat: number,
    centerLon: number,
    radius: number,
    unit: DistanceUnit = 'm',
    options?: {
      withDist?: boolean;
      withCoord?: boolean;
      count?: number;
      sort?: 'ASC' | 'DESC';
    },
    commandOptions?: CommandOptions
  ): Promise<GeoradiusResult[]> {
    if (centerLat < -90 || centerLat > 90) {
      throw new TypeError(`Latitude must be between -90 and 90, got: ${centerLat}`);
    }
    if (centerLon < -180 || centerLon > 180) {
      throw new TypeError(`Longitude must be between -180 and 180, got: ${centerLon}`);
    }

    const response = await this.client.sendCommand<GeoradiusResponse>(
      'geospatial.georadius',
      this.buildPayload(commandOptions, {
        key,
        center_lat: centerLat,
        center_lon: centerLon,
        radius,
        unit,
        with_dist: options?.withDist ?? false,
        with_coord: options?.withCoord ?? false,
        count: options?.count,
        sort: options?.sort,
      })
    );

    return response.results;
  }

  /**
   * Query members within radius of given member (GEORADIUSBYMEMBER)
   * @param key Geospatial key
   * @param member Center member
   * @param radius Radius
   * @param unit Distance unit (default: 'm')
   * @param options Query options (withDist, withCoord, count, sort)
   * @param commandOptions Optional command options
   * @returns Array of matching members with optional distance and coordinates
   */
  async georadiusbymember(
    key: string,
    member: string,
    radius: number,
    unit: DistanceUnit = 'm',
    options?: {
      withDist?: boolean;
      withCoord?: boolean;
      count?: number;
      sort?: 'ASC' | 'DESC';
    },
    commandOptions?: CommandOptions
  ): Promise<GeoradiusResult[]> {
    const response = await this.client.sendCommand<GeoradiusResponse>(
      'geospatial.georadiusbymember',
      this.buildPayload(commandOptions, {
        key,
        member,
        radius,
        unit,
        with_dist: options?.withDist ?? false,
        with_coord: options?.withCoord ?? false,
        count: options?.count,
        sort: options?.sort,
      })
    );

    return response.results;
  }

  /**
   * Get coordinates of members (GEOPOS)
   * @param key Geospatial key
   * @param members Array of member names
   * @param options Optional command options
   * @returns Array of coordinates (null if member doesn't exist)
   */
  async geopos(
    key: string,
    members: string[],
    options?: CommandOptions
  ): Promise<(Coordinate | null)[]> {
    const response = await this.client.sendCommand<GeoposResponse>(
      'geospatial.geopos',
      this.buildPayload(options, {
        key,
        members,
      })
    );

    return response.coordinates;
  }

  /**
   * Get geohash strings for members (GEOHASH)
   * @param key Geospatial key
   * @param members Array of member names
   * @param options Optional command options
   * @returns Array of geohash strings (null if member doesn't exist)
   */
  async geohash(
    key: string,
    members: string[],
    options?: CommandOptions
  ): Promise<(string | null)[]> {
    const response = await this.client.sendCommand<GeohashResponse>(
      'geospatial.geohash',
      this.buildPayload(options, {
        key,
        members,
      })
    );

    return response.geohashes;
  }

  /**
   * Advanced geospatial search (GEOSEARCH)
   * @param key Geospatial key
   * @param options Search options (fromMember/fromLonLat, byRadius/byBox, withDist, withCoord, count, sort)
   * @param commandOptions Optional command options
   * @returns Array of matching members with optional distance and coordinates
   */
  async geosearch(
    key: string,
    options: {
      fromMember?: string;
      fromLonLat?: [number, number]; // [lon, lat]
      byRadius?: [number, DistanceUnit];
      byBox?: [number, number, DistanceUnit]; // [width, height, unit]
      withDist?: boolean;
      withCoord?: boolean;
      withHash?: boolean;
      count?: number;
      sort?: 'ASC' | 'DESC';
    },
    commandOptions?: CommandOptions
  ): Promise<GeoradiusResult[]> {
    if (!options.fromMember && !options.fromLonLat) {
      throw new TypeError("Either 'fromMember' or 'fromLonLat' must be provided");
    }
    if (!options.byRadius && !options.byBox) {
      throw new TypeError("Either 'byRadius' or 'byBox' must be provided");
    }

    const payload: Record<string, unknown> = {
      key,
      with_dist: options.withDist ?? false,
      with_coord: options.withCoord ?? false,
      with_hash: options.withHash ?? false,
    };

    if (options.fromMember) {
      payload.from_member = options.fromMember;
    }
    if (options.fromLonLat) {
      payload.from_lonlat = options.fromLonLat;
    }
    if (options.byRadius) {
      payload.by_radius = [options.byRadius[0], options.byRadius[1]];
    }
    if (options.byBox) {
      payload.by_box = [options.byBox[0], options.byBox[1], options.byBox[2]];
    }
    if (options.count !== undefined) {
      payload.count = options.count;
    }
    if (options.sort) {
      payload.sort = options.sort;
    }

    const response = await this.client.sendCommand<GeoradiusResponse>(
      'geospatial.geosearch',
      this.buildPayload(commandOptions, payload)
    );

    return response.results;
  }

  /**
   * Retrieve geospatial statistics
   * @param options Optional command options
   * @returns Geospatial statistics
   */
  async stats(options?: CommandOptions): Promise<GeospatialStats> {
    const response = await this.client.sendCommand<GeospatialStats>(
      'geospatial.stats',
      this.buildPayload(options, {})
    );

    return response;
  }
}

