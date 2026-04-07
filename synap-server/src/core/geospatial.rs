//! Geospatial data structure implementation for Synap
//!
//! Provides Redis-compatible geospatial operations (GEOADD, GEODIST, GEORADIUS, etc.)
//! Storage: Backed by Sorted Sets with geohash-encoded scores
//!
//! # Architecture
//! ```text
//! GeospatialStore
//!   ├─ Uses SortedSetStore internally
//!   ├─ Converts lat/lon to geohash score
//!   └─ Provides geospatial query operations
//! ```
//!
//! # How Redis Stores Geodata
//! Redis uses Sorted Sets where:
//! - Member = location name (string)
//! - Score = geohash encoded as 52-bit integer
//!
//! The geohash is encoded as: `(lat + 90.0) * (1 << 26) + (lon + 180.0)`

use super::error::{Result, SynapError};
use super::sorted_set::SortedSetStore;
use geohash::{Coord, encode};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const EARTH_RADIUS_KM: f64 = 6371.0;
const EARTH_RADIUS_M: f64 = 6371000.0;
const EARTH_RADIUS_MI: f64 = 3959.0;
const EARTH_RADIUS_FT: f64 = 20902231.0;

/// Distance unit for GEODIST
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    Meters,
    Kilometers,
    Miles,
    Feet,
}

impl DistanceUnit {
    /// Get Earth radius for this unit
    fn radius(&self) -> f64 {
        match self {
            DistanceUnit::Meters => EARTH_RADIUS_M,
            DistanceUnit::Kilometers => EARTH_RADIUS_KM,
            DistanceUnit::Miles => EARTH_RADIUS_MI,
            DistanceUnit::Feet => EARTH_RADIUS_FT,
        }
    }
}

impl std::str::FromStr for DistanceUnit {
    type Err = SynapError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "M" | "METER" | "METERS" => Ok(DistanceUnit::Meters),
            "KM" | "KILOMETER" | "KILOMETERS" => Ok(DistanceUnit::Kilometers),
            "MI" | "MILE" | "MILES" => Ok(DistanceUnit::Miles),
            "FT" | "FOOT" | "FEET" => Ok(DistanceUnit::Feet),
            _ => Err(SynapError::InvalidValue(format!(
                "Invalid distance unit: {}. Must be one of: m, km, mi, ft",
                s
            ))),
        }
    }
}

/// Coordinate (latitude, longitude)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinate {
    pub lat: f64,
    pub lon: f64,
}

impl Coordinate {
    pub fn new(lat: f64, lon: f64) -> Result<Self> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(SynapError::InvalidValue(format!(
                "Latitude must be between -90 and 90, got: {}",
                lat
            )));
        }
        if !(-180.0..=180.0).contains(&lon) {
            return Err(SynapError::InvalidValue(format!(
                "Longitude must be between -180 and 180, got: {}",
                lon
            )));
        }
        Ok(Self { lat, lon })
    }
}

/// Convert coordinate to geohash score (Redis-compatible encoding)
/// Redis uses: (lat + 90.0) * (1 << 26) + (lon + 180.0)
/// This produces a 52-bit integer stored as f64
fn coordinate_to_score(lat: f64, lon: f64) -> f64 {
    // Normalize to [0, 180) for lat and [0, 360) for lon
    let normalized_lat = (lat + 90.0).clamp(0.0, 180.0);
    let normalized_lon = (lon + 180.0).clamp(0.0, 360.0);

    // Encode as 52-bit integer (26 bits for lat, 26 bits for lon)
    // Normalize to integer range: [0, 180*2^26) for lat, [0, 360*2^26) for lon
    // But we need to fit lon in 26 bits, so scale it to [0, 2^26)
    let lat_scaled = (normalized_lat * (1u64 << 26) as f64 / 180.0).round() as u64;
    let lon_scaled = (normalized_lon * (1u64 << 26) as f64 / 360.0).round() as u64;

    // Combine: lat in upper 26 bits, lon in lower 26 bits
    ((lat_scaled << 26) | lon_scaled) as f64
}

/// Convert geohash score back to coordinate
fn score_to_coordinate(score: f64) -> Coordinate {
    // Extract lat and lon from encoded score
    // Score format: (lat_scaled << 26) | lon_scaled
    let bits = score as u64;
    let lat_scaled = bits >> 26;
    let lon_scaled = bits & ((1u64 << 26) - 1);

    // Denormalize: scale back from [0, 2^26) to [0, 180) and [0, 360)
    let normalized_lat = lat_scaled as f64 * 180.0 / (1u64 << 26) as f64;
    let normalized_lon = lon_scaled as f64 * 360.0 / (1u64 << 26) as f64;

    // Convert back to original ranges
    let lat = normalized_lat - 90.0;
    let lon = normalized_lon - 180.0;

    Coordinate { lat, lon }
}

/// Calculate Haversine distance between two coordinates
fn haversine_distance(coord1: Coordinate, coord2: Coordinate, unit: DistanceUnit) -> f64 {
    let lat1 = coord1.lat.to_radians();
    let lon1 = coord1.lon.to_radians();
    let lat2 = coord2.lat.to_radians();
    let lon2 = coord2.lon.to_radians();

    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;

    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    c * unit.radius()
}

/// Result type for georadius queries
pub type GeospatialRadiusResult = (Vec<u8>, Option<f64>, Option<Coordinate>);

/// Geospatial statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeospatialStats {
    pub total_keys: usize,
    pub total_locations: usize,
    pub geoadd_count: usize,
    pub geodist_count: usize,
    pub georadius_count: usize,
    pub geopos_count: usize,
    pub geohash_count: usize,
}

/// Geospatial store backed by Sorted Sets
pub struct GeospatialStore {
    sorted_set_store: Arc<SortedSetStore>,
    stats: Arc<RwLock<GeospatialStats>>,
}

impl GeospatialStore {
    /// Create a new geospatial store
    pub fn new(sorted_set_store: Arc<SortedSetStore>) -> Self {
        Self {
            sorted_set_store,
            stats: Arc::new(RwLock::new(GeospatialStats {
                total_keys: 0,
                total_locations: 0,
                geoadd_count: 0,
                geodist_count: 0,
                georadius_count: 0,
                geopos_count: 0,
                geohash_count: 0,
            })),
        }
    }

    /// GEOADD - Add one or more geospatial items (latitude, longitude, member)
    /// Returns the number of elements added (not updated)
    pub fn geoadd(
        &self,
        key: &str,
        locations: Vec<(f64, f64, Vec<u8>)>,
        nx: bool,
        xx: bool,
        ch: bool,
    ) -> Result<usize> {
        // Validate coordinates
        for (lat, lon, _) in &locations {
            Coordinate::new(*lat, *lon)?;
        }

        let locations_count = locations.len();
        let mut added = 0;

        // Check if key exists before adding (for total_keys tracking)
        let key_existed = self.sorted_set_store.zcard(key) > 0;

        for (lat, lon, member) in locations {
            let score = coordinate_to_score(lat, lon);

            let mut opts = super::sorted_set::ZAddOptions::default();
            opts.nx = nx;
            opts.xx = xx;
            opts.ch = ch;

            let (added_count, _changed_count) =
                self.sorted_set_store.zadd(key, member, score, &opts);
            added += added_count;
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.geoadd_count += locations_count;
            if added > 0 {
                stats.total_locations += added;
            }
            // If key didn't exist before and we added at least one item, increment total_keys
            if !key_existed && added > 0 {
                stats.total_keys += 1;
            }
        }

        Ok(added)
    }

    /// GEODIST - Calculate distance between two members
    /// Returns distance in requested unit, or None if either member doesn't exist
    pub fn geodist(
        &self,
        key: &str,
        member1: &[u8],
        member2: &[u8],
        unit: DistanceUnit,
    ) -> Result<Option<f64>> {
        let coord1 = self.get_coordinate(key, member1)?;
        let coord2 = self.get_coordinate(key, member2)?;

        let coord1 = match coord1 {
            Some(c) => c,
            None => return Ok(None),
        };
        let coord2 = match coord2 {
            Some(c) => c,
            None => return Ok(None),
        };

        let distance = haversine_distance(coord1, coord2, unit);

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.geodist_count += 1;
        }

        Ok(Some(distance))
    }

    /// Get coordinate for a member
    fn get_coordinate(&self, key: &str, member: &[u8]) -> Result<Option<Coordinate>> {
        let score = self.sorted_set_store.zscore(key, member);
        Ok(score.map(score_to_coordinate))
    }

    /// GEOPOS - Get coordinates of one or more members
    /// Returns vector of Option<Coordinate> (None if member doesn't exist)
    pub fn geopos(&self, key: &str, members: &[Vec<u8>]) -> Result<Vec<Option<Coordinate>>> {
        let mut results = Vec::new();

        for member in members {
            let coord = self.get_coordinate(key, member)?;
            results.push(coord);
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.geopos_count += members.len();
        }

        Ok(results)
    }

    /// GEOHASH - Get geohash string for one or more members
    /// Returns vector of Option<String> (None if member doesn't exist)
    pub fn geohash(&self, key: &str, members: &[Vec<u8>]) -> Result<Vec<Option<String>>> {
        let mut results = Vec::new();

        for member in members {
            let coord = self.get_coordinate(key, member)?;
            if let Some(coord) = coord {
                // Use geohash crate to encode
                let coord_obj = Coord {
                    x: coord.lon,
                    y: coord.lat,
                };
                // Redis uses 11-character geohash by default
                // encode returns Result, unwrap for now (coordinates are already validated)
                match encode(coord_obj, 11) {
                    Ok(hash) => results.push(Some(hash)),
                    Err(_) => results.push(None), // Should not happen with valid coordinates
                }
            } else {
                results.push(None);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.geohash_count += members.len();
        }

        Ok(results)
    }

    /// GEORADIUS - Query members within radius of given coordinate
    /// Returns vector of (member, distance) pairs sorted by distance
    #[allow(clippy::too_many_arguments)]
    pub fn georadius(
        &self,
        key: &str,
        center_lat: f64,
        center_lon: f64,
        radius: f64,
        unit: DistanceUnit,
        with_dist: bool,
        with_coord: bool,
        count: Option<usize>,
        sort: Option<&str>,
    ) -> Result<Vec<GeospatialRadiusResult>> {
        let center = Coordinate::new(center_lat, center_lon)?;
        let radius_meters = match unit {
            DistanceUnit::Meters => radius,
            DistanceUnit::Kilometers => radius * 1000.0,
            DistanceUnit::Miles => radius * 1609.34,
            DistanceUnit::Feet => radius * 0.3048,
        };

        // Get all members with their scores
        let all_members = self.sorted_set_store.zrange(key, 0, -1, true);
        let mut results = Vec::new();

        // Debug: Check if we have members and verify coordinate encoding/decoding
        if all_members.is_empty() {
            return Ok(results);
        }

        for scored_member in all_members {
            let member_coord = score_to_coordinate(scored_member.score);
            let distance_meters = haversine_distance(center, member_coord, DistanceUnit::Meters);

            if distance_meters <= radius_meters {
                let distance = if with_dist {
                    Some(haversine_distance(center, member_coord, unit))
                } else {
                    None
                };
                let coord = if with_coord { Some(member_coord) } else { None };
                results.push((scored_member.member, distance, coord));
            }
        }

        // Sort by distance (ascending by default)
        if let Some(sort_str) = sort {
            match sort_str.to_uppercase().as_str() {
                "ASC" => {
                    results.sort_by(|a, b| {
                        let dist_a = a.1.unwrap_or(f64::INFINITY);
                        let dist_b = b.1.unwrap_or(f64::INFINITY);
                        dist_a
                            .partial_cmp(&dist_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                "DESC" => {
                    results.sort_by(|a, b| {
                        let dist_a = a.1.unwrap_or(f64::INFINITY);
                        let dist_b = b.1.unwrap_or(f64::INFINITY);
                        dist_b
                            .partial_cmp(&dist_a)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                _ => {} // No sort
            }
        } else {
            // Default: sort by distance ascending
            results.sort_by(|a, b| {
                let dist_a = a.1.unwrap_or(f64::INFINITY);
                let dist_b = b.1.unwrap_or(f64::INFINITY);
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Apply count limit
        if let Some(limit) = count {
            results.truncate(limit);
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.georadius_count += 1;
        }

        Ok(results)
    }

    /// GEORADIUSBYMEMBER - Query members within radius of given member
    /// Returns vector of (member, distance) pairs sorted by distance
    #[allow(clippy::too_many_arguments)]
    pub fn georadiusbymember(
        &self,
        key: &str,
        member: &[u8],
        radius: f64,
        unit: DistanceUnit,
        with_dist: bool,
        with_coord: bool,
        count: Option<usize>,
        sort: Option<&str>,
    ) -> Result<Vec<GeospatialRadiusResult>> {
        // Get center coordinate from member
        let center_coord = self.get_coordinate(key, member)?;
        let center = match center_coord {
            Some(c) => c,
            None => {
                return Err(SynapError::KeyNotFound(format!(
                    "Member not found in geospatial key: {}",
                    String::from_utf8_lossy(member)
                )));
            }
        };

        // Delegate to georadius
        self.georadius(
            key, center.lat, center.lon, radius, unit, with_dist, with_coord, count, sort,
        )
    }

    /// GEOSEARCH - Advanced geospatial search with FROMMEMBER/FROMLONLAT and BYRADIUS/BYBOX
    /// Supports both radius-based and bounding box queries
    #[allow(clippy::too_many_arguments)]
    pub fn geosearch(
        &self,
        key: &str,
        from_member: Option<&[u8]>,
        from_lonlat: Option<(f64, f64)>,
        by_radius: Option<(f64, DistanceUnit)>,
        by_box: Option<(f64, f64, DistanceUnit)>,
        with_dist: bool,
        with_coord: bool,
        _with_hash: bool,
        count: Option<usize>,
        sort: Option<&str>,
    ) -> Result<Vec<GeospatialRadiusResult>> {
        // Determine center coordinate
        let center = if let Some(member) = from_member {
            let coord = self.get_coordinate(key, member)?;
            coord.ok_or_else(|| {
                SynapError::KeyNotFound(format!(
                    "Member not found in geospatial key: {}",
                    String::from_utf8_lossy(member)
                ))
            })?
        } else if let Some((lon, lat)) = from_lonlat {
            Coordinate::new(lat, lon)?
        } else {
            return Err(SynapError::InvalidRequest(
                "Either 'from_member' or 'from_lonlat' must be provided".to_string(),
            ));
        };

        // Get all members with their scores
        let all_members = self.sorted_set_store.zrange(key, 0, -1, true);
        let mut results = Vec::new();

        if all_members.is_empty() {
            return Ok(results);
        }

        // Filter by radius or bounding box
        for scored_member in all_members {
            let member_coord = score_to_coordinate(scored_member.score);
            let include = if let Some((radius, unit)) = by_radius {
                let radius_meters = match unit {
                    DistanceUnit::Meters => radius,
                    DistanceUnit::Kilometers => radius * 1000.0,
                    DistanceUnit::Miles => radius * 1609.34,
                    DistanceUnit::Feet => radius * 0.3048,
                };
                let distance_meters =
                    haversine_distance(center, member_coord, DistanceUnit::Meters);
                distance_meters <= radius_meters
            } else if let Some((width, height, unit)) = by_box {
                // Convert box dimensions to meters
                let width_meters = match unit {
                    DistanceUnit::Meters => width,
                    DistanceUnit::Kilometers => width * 1000.0,
                    DistanceUnit::Miles => width * 1609.34,
                    DistanceUnit::Feet => width * 0.3048,
                };
                let height_meters = match unit {
                    DistanceUnit::Meters => height,
                    DistanceUnit::Kilometers => height * 1000.0,
                    DistanceUnit::Miles => height * 1609.34,
                    DistanceUnit::Feet => height * 0.3048,
                };

                // Calculate bounding box
                // Approximate: 1 degree lat ≈ 111km, 1 degree lon ≈ 111km * cos(lat)
                let lat_rad = center.lat.to_radians();
                let lat_degrees_per_meter = 1.0 / 111000.0;
                let lon_degrees_per_meter = 1.0 / (111000.0 * lat_rad.cos());

                let half_width_deg = (width_meters / 2.0) * lon_degrees_per_meter;
                let half_height_deg = (height_meters / 2.0) * lat_degrees_per_meter;

                let min_lon = center.lon - half_width_deg;
                let max_lon = center.lon + half_width_deg;
                let min_lat = center.lat - half_height_deg;
                let max_lat = center.lat + half_height_deg;

                member_coord.lon >= min_lon
                    && member_coord.lon <= max_lon
                    && member_coord.lat >= min_lat
                    && member_coord.lat <= max_lat
            } else {
                return Err(SynapError::InvalidRequest(
                    "Either 'by_radius' or 'by_box' must be provided".to_string(),
                ));
            };

            if include {
                let distance = if with_dist {
                    Some(haversine_distance(
                        center,
                        member_coord,
                        by_radius.map(|(_, u)| u).unwrap_or(DistanceUnit::Meters),
                    ))
                } else {
                    None
                };
                let coord = if with_coord { Some(member_coord) } else { None };
                results.push((scored_member.member, distance, coord));
            }
        }

        // Sort by distance (ascending by default)
        if let Some(sort_str) = sort {
            match sort_str.to_uppercase().as_str() {
                "ASC" => {
                    results.sort_by(|a, b| {
                        let dist_a = a.1.unwrap_or(f64::INFINITY);
                        let dist_b = b.1.unwrap_or(f64::INFINITY);
                        dist_a
                            .partial_cmp(&dist_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                "DESC" => {
                    results.sort_by(|a, b| {
                        let dist_a = a.1.unwrap_or(f64::INFINITY);
                        let dist_b = b.1.unwrap_or(f64::INFINITY);
                        dist_b
                            .partial_cmp(&dist_a)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                _ => {} // No sort
            }
        } else {
            // Default: sort by distance ascending
            results.sort_by(|a, b| {
                let dist_a = a.1.unwrap_or(f64::INFINITY);
                let dist_b = b.1.unwrap_or(f64::INFINITY);
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Apply count limit
        if let Some(limit) = count {
            results.truncate(limit);
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.georadius_count += 1; // Use georadius_count for GEOSEARCH too
        }

        Ok(results)
    }

    /// Get statistics
    pub fn stats(&self) -> GeospatialStats {
        self.stats.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sorted_set::SortedSetStore;

    fn create_store() -> GeospatialStore {
        let sorted_set_store = Arc::new(SortedSetStore::new());
        GeospatialStore::new(sorted_set_store)
    }

    #[test]
    fn test_geoadd_basic() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        let added = store
            .geoadd("cities", locations, false, false, false)
            .unwrap();
        assert_eq!(added, 1);
    }

    #[test]
    fn test_geoadd_multiple() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (40.7128, -74.0060, b"New York".to_vec()),
            (34.0522, -118.2437, b"Los Angeles".to_vec()),
        ];
        let added = store
            .geoadd("cities", locations, false, false, false)
            .unwrap();
        assert_eq!(added, 3);
    }

    #[test]
    fn test_geoadd_nx_only_new() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations.clone(), false, false, false)
            .unwrap();

        // Try to add same member with nx=true (should not update)
        let added = store
            .geoadd("cities", locations, true, false, false)
            .unwrap();
        assert_eq!(added, 0);
    }

    #[test]
    fn test_geoadd_xx_only_existing() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];

        // Try to add with xx=true (should not add new)
        let added = store
            .geoadd("cities", locations, false, true, false)
            .unwrap();
        assert_eq!(added, 0);

        // Add first, then update with xx=true
        store
            .geoadd(
                "cities",
                vec![(37.7749, -122.4194, b"San Francisco".to_vec())],
                false,
                false,
                false,
            )
            .unwrap();
        let updated = store
            .geoadd(
                "cities",
                vec![(37.7750, -122.4195, b"San Francisco".to_vec())],
                false,
                true,
                false,
            )
            .unwrap();
        assert_eq!(updated, 0); // xx doesn't add, only updates existing
    }

    #[test]
    fn test_geoadd_invalid_coordinates() {
        let store = create_store();
        let locations = vec![(91.0, -122.4194, b"Invalid".to_vec())];
        let result = store.geoadd("cities", locations, false, false, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_geodist_same_location() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let distance = store
            .geodist(
                "cities",
                b"San Francisco",
                b"San Francisco",
                DistanceUnit::Meters,
            )
            .unwrap();
        assert_eq!(distance, Some(0.0));
    }

    #[test]
    fn test_geodist_different_locations() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (40.7128, -74.0060, b"New York".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let distance = store
            .geodist(
                "cities",
                b"San Francisco",
                b"New York",
                DistanceUnit::Kilometers,
            )
            .unwrap();
        assert!(distance.is_some());
        assert!(distance.unwrap() > 4000.0); // SF to NY is ~4100km
        assert!(distance.unwrap() < 4200.0);
    }

    #[test]
    fn test_geodist_member_not_found() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let distance = store
            .geodist("cities", b"San Francisco", b"Unknown", DistanceUnit::Meters)
            .unwrap();
        assert_eq!(distance, None);
    }

    #[test]
    fn test_geopos_single() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let coords = store
            .geopos("cities", &[b"San Francisco".to_vec()])
            .unwrap();
        assert_eq!(coords.len(), 1);
        assert!(coords[0].is_some());
        let coord = coords[0].unwrap();
        assert!((coord.lat - 37.7749).abs() < 0.01);
        assert!((coord.lon - (-122.4194)).abs() < 0.01);
    }

    #[test]
    fn test_geopos_multiple() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (40.7128, -74.0060, b"New York".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let coords = store
            .geopos("cities", &[b"San Francisco".to_vec(), b"New York".to_vec()])
            .unwrap();
        assert_eq!(coords.len(), 2);
        assert!(coords[0].is_some());
        assert!(coords[1].is_some());
    }

    #[test]
    fn test_geopos_not_found() {
        let store = create_store();
        let coords = store.geopos("cities", &[b"Unknown".to_vec()]).unwrap();
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], None);
    }

    #[test]
    fn test_geohash_single() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let hashes = store
            .geohash("cities", &[b"San Francisco".to_vec()])
            .unwrap();
        assert_eq!(hashes.len(), 1);
        assert!(hashes[0].is_some());
        assert_eq!(hashes[0].as_ref().unwrap().len(), 11); // Redis uses 11-char geohash
    }

    #[test]
    fn test_georadius_within_radius() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (37.7849, -122.4094, b"Near SF".to_vec()), // ~10km away
            (40.7128, -74.0060, b"New York".to_vec()), // ~4100km away
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .georadius(
                "cities",
                37.7749,
                -122.4194,
                50.0,
                DistanceUnit::Kilometers,
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 2); // SF and Near SF
    }

    #[test]
    fn test_georadius_with_dist() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .georadius(
                "cities",
                37.7749,
                -122.4194,
                10.0,
                DistanceUnit::Kilometers,
                true,
                false,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_some()); // distance should be present
        assert!(results[0].1.unwrap() < 1.0); // Should be very close to 0
    }

    #[test]
    fn test_georadius_with_coord() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .georadius(
                "cities",
                37.7749,
                -122.4194,
                10.0,
                DistanceUnit::Kilometers,
                false,
                true,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].2.is_some()); // coord should be present
    }

    #[test]
    fn test_georadius_count_limit() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"SF1".to_vec()),
            (37.7750, -122.4195, b"SF2".to_vec()),
            (37.7751, -122.4196, b"SF3".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .georadius(
                "cities",
                37.7749,
                -122.4194,
                10.0,
                DistanceUnit::Kilometers,
                false,
                false,
                Some(2),
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_georadiusbymember() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (37.7849, -122.4094, b"Near SF".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .georadiusbymember(
                "cities",
                b"San Francisco",
                50.0,
                DistanceUnit::Kilometers,
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_geosearch_from_member_by_radius() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (37.7849, -122.4094, b"Near SF".to_vec()),
            (40.7128, -74.0060, b"New York".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .geosearch(
                "cities",
                Some(b"San Francisco"),
                None,
                Some((50.0, DistanceUnit::Kilometers)),
                None,
                false,
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 2); // SF and Near SF
    }

    #[test]
    fn test_geosearch_from_lonlat_by_radius() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (37.7849, -122.4094, b"Near SF".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        let results = store
            .geosearch(
                "cities",
                None,
                Some((-122.4194, 37.7749)),
                Some((50.0, DistanceUnit::Kilometers)),
                None,
                false,
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_geosearch_by_box() {
        let store = create_store();
        let locations = vec![
            (37.7749, -122.4194, b"San Francisco".to_vec()),
            (37.7849, -122.4094, b"Near SF".to_vec()),
            (40.7128, -74.0060, b"New York".to_vec()),
        ];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();

        // Box of 100km x 100km around SF
        let results = store
            .geosearch(
                "cities",
                Some(b"San Francisco"),
                None,
                None,
                Some((100000.0, 100000.0, DistanceUnit::Meters)),
                false,
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert!(results.len() >= 2); // Should include SF and Near SF
    }

    #[test]
    fn test_geosearch_invalid_from() {
        let store = create_store();
        let result = store.geosearch(
            "cities",
            None,
            None,
            Some((50.0, DistanceUnit::Kilometers)),
            None,
            false,
            false,
            false,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_geosearch_invalid_by() {
        let store = create_store();
        let result = store.geosearch(
            "cities",
            Some(b"San Francisco"),
            None,
            None,
            None,
            false,
            false,
            false,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_stats() {
        let store = create_store();
        let locations = vec![(37.7749, -122.4194, b"San Francisco".to_vec())];
        store
            .geoadd("cities", locations, false, false, false)
            .unwrap();
        store
            .geodist(
                "cities",
                b"San Francisco",
                b"San Francisco",
                DistanceUnit::Meters,
            )
            .unwrap();

        let stats = store.stats();
        assert_eq!(stats.geoadd_count, 1);
        assert_eq!(stats.geodist_count, 1);
    }
}
