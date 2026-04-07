//! Geospatial operations (GEOADD/GEODIST/GEORADIUS/GEOPOS/GEOHASH)

use crate::client::SynapClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Distance unit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    Meters,
    Kilometers,
    Miles,
    Feet,
}

impl DistanceUnit {
    fn as_str(&self) -> &'static str {
        match self {
            DistanceUnit::Meters => "m",
            DistanceUnit::Kilometers => "km",
            DistanceUnit::Miles => "mi",
            DistanceUnit::Feet => "ft",
        }
    }
}

/// Location coordinate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
    pub member: String,
}

/// Coordinate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinate {
    pub lat: f64,
    pub lon: f64,
}

/// Georadius result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoradiusResult {
    pub member: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coord: Option<Coordinate>,
}

/// Geospatial statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeospatialStats {
    pub total_keys: usize,
    pub total_locations: usize,
    pub geoadd_count: usize,
    pub geodist_count: usize,
    pub georadius_count: usize,
    pub geopos_count: usize,
    pub geohash_count: usize,
}

#[derive(Clone)]
pub struct GeospatialManager {
    client: SynapClient,
}

impl GeospatialManager {
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Add geospatial locations (GEOADD)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `locations` - Array of locations (lat, lon, member)
    /// * `nx` - Only add new elements (don't update existing)
    /// * `xx` - Only update existing elements (don't add new)
    /// * `ch` - Return count of changed elements
    ///
    /// # Returns
    ///
    /// Number of elements added
    pub async fn geoadd(
        &self,
        key: &str,
        locations: Vec<Location>,
        nx: bool,
        xx: bool,
        ch: bool,
    ) -> Result<usize> {
        // Validate coordinates
        for loc in &locations {
            if !(-90.0..=90.0).contains(&loc.lat) {
                return Err(crate::error::SynapError::ServerError(format!(
                    "Latitude must be between -90 and 90, got: {}",
                    loc.lat
                )));
            }
            if !(-180.0..=180.0).contains(&loc.lon) {
                return Err(crate::error::SynapError::ServerError(format!(
                    "Longitude must be between -180 and 180, got: {}",
                    loc.lon
                )));
            }
        }

        let payload = json!({
            "key": key,
            "locations": locations,
            "nx": nx,
            "xx": xx,
            "ch": ch,
        });

        let response = self
            .client
            .send_command("geospatial.geoadd", payload)
            .await?;
        Ok(response["added"].as_u64().unwrap_or(0) as usize)
    }

    /// Calculate distance between two members (GEODIST)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `member1` - First member
    /// * `member2` - Second member
    /// * `unit` - Distance unit
    ///
    /// # Returns
    ///
    /// Distance in specified unit, or None if either member doesn't exist
    pub async fn geodist(
        &self,
        key: &str,
        member1: &str,
        member2: &str,
        unit: DistanceUnit,
    ) -> Result<Option<f64>> {
        let payload = json!({
            "key": key,
            "member1": member1,
            "member2": member2,
            "unit": unit.as_str(),
        });

        let response = self
            .client
            .send_command("geospatial.geodist", payload)
            .await?;
        let distance = response["distance"].as_f64();
        Ok(distance)
    }

    /// Query members within radius (GEORADIUS)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `center_lat` - Center latitude
    /// * `center_lon` - Center longitude
    /// * `radius` - Radius
    /// * `unit` - Distance unit
    /// * `with_dist` - Include distance in results
    /// * `with_coord` - Include coordinates in results
    /// * `count` - Maximum number of results
    /// * `sort` - Sort order ("ASC" or "DESC")
    ///
    /// # Returns
    ///
    /// Vector of matching members with optional distance and coordinates
    #[allow(clippy::too_many_arguments)]
    pub async fn georadius(
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
    ) -> Result<Vec<GeoradiusResult>> {
        if !(-90.0..=90.0).contains(&center_lat) {
            return Err(crate::error::SynapError::ServerError(format!(
                "Latitude must be between -90 and 90, got: {}",
                center_lat
            )));
        }
        if !(-180.0..=180.0).contains(&center_lon) {
            return Err(crate::error::SynapError::ServerError(format!(
                "Longitude must be between -180 and 180, got: {}",
                center_lon
            )));
        }

        let mut payload = json!({
            "key": key,
            "center_lat": center_lat,
            "center_lon": center_lon,
            "radius": radius,
            "unit": unit.as_str(),
            "with_dist": with_dist,
            "with_coord": with_coord,
        });

        if let Some(c) = count {
            payload["count"] = json!(c);
        }
        if let Some(s) = sort {
            payload["sort"] = json!(s);
        }

        let response = self
            .client
            .send_command("geospatial.georadius", payload)
            .await?;
        let results: Vec<GeoradiusResult> =
            serde_json::from_value(response["results"].clone()).unwrap_or_default();
        Ok(results)
    }

    /// Query members within radius of given member (GEORADIUSBYMEMBER)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `member` - Center member
    /// * `radius` - Radius
    /// * `unit` - Distance unit
    /// * `with_dist` - Include distance in results
    /// * `with_coord` - Include coordinates in results
    /// * `count` - Maximum number of results
    /// * `sort` - Sort order ("ASC" or "DESC")
    ///
    /// # Returns
    ///
    /// Vector of matching members with optional distance and coordinates
    #[allow(clippy::too_many_arguments)]
    pub async fn georadiusbymember(
        &self,
        key: &str,
        member: &str,
        radius: f64,
        unit: DistanceUnit,
        with_dist: bool,
        with_coord: bool,
        count: Option<usize>,
        sort: Option<&str>,
    ) -> Result<Vec<GeoradiusResult>> {
        let mut payload = json!({
            "key": key,
            "member": member,
            "radius": radius,
            "unit": unit.as_str(),
            "with_dist": with_dist,
            "with_coord": with_coord,
        });

        if let Some(c) = count {
            payload["count"] = json!(c);
        }
        if let Some(s) = sort {
            payload["sort"] = json!(s);
        }

        let response = self
            .client
            .send_command("geospatial.georadiusbymember", payload)
            .await?;
        let results: Vec<GeoradiusResult> =
            serde_json::from_value(response["results"].clone()).unwrap_or_default();
        Ok(results)
    }

    /// Get coordinates of members (GEOPOS)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `members` - Array of member names
    ///
    /// # Returns
    ///
    /// Vector of coordinates (None if member doesn't exist)
    pub async fn geopos(&self, key: &str, members: &[String]) -> Result<Vec<Option<Coordinate>>> {
        let payload = json!({
            "key": key,
            "members": members,
        });

        let response = self
            .client
            .send_command("geospatial.geopos", payload)
            .await?;
        let coords: Vec<Option<Coordinate>> =
            serde_json::from_value(response["coordinates"].clone()).unwrap_or_default();
        Ok(coords)
    }

    /// Advanced geospatial search (GEOSEARCH)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `from_member` - Center member (mutually exclusive with from_lonlat)
    /// * `from_lonlat` - Center coordinates as (lon, lat) tuple (mutually exclusive with from_member)
    /// * `by_radius` - Search by radius as (radius, unit) tuple
    /// * `by_box` - Search by bounding box as (width, height, unit) tuple
    /// * `with_dist` - Include distance in results
    /// * `with_coord` - Include coordinates in results
    /// * `with_hash` - Include geohash in results (not yet implemented)
    /// * `count` - Maximum number of results
    /// * `sort` - Sort order ("ASC" or "DESC")
    ///
    /// # Returns
    ///
    /// Vector of matching members with optional distance and coordinates
    #[allow(clippy::too_many_arguments)]
    pub async fn geosearch(
        &self,
        key: &str,
        from_member: Option<&str>,
        from_lonlat: Option<(f64, f64)>,
        by_radius: Option<(f64, DistanceUnit)>,
        by_box: Option<(f64, f64, DistanceUnit)>,
        with_dist: bool,
        with_coord: bool,
        with_hash: bool,
        count: Option<usize>,
        sort: Option<&str>,
    ) -> Result<Vec<GeoradiusResult>> {
        if from_member.is_none() && from_lonlat.is_none() {
            return Err(crate::error::SynapError::ServerError(
                "Either 'from_member' or 'from_lonlat' must be provided".to_string(),
            ));
        }
        if by_radius.is_none() && by_box.is_none() {
            return Err(crate::error::SynapError::ServerError(
                "Either 'by_radius' or 'by_box' must be provided".to_string(),
            ));
        }

        let mut payload = json!({
            "key": key,
            "with_dist": with_dist,
            "with_coord": with_coord,
            "with_hash": with_hash,
        });

        if let Some(member) = from_member {
            payload["from_member"] = json!(member);
        }
        if let Some((lon, lat)) = from_lonlat {
            payload["from_lonlat"] = json!([lon, lat]);
        }
        if let Some((radius, unit)) = by_radius {
            payload["by_radius"] = json!([radius, unit.as_str()]);
        }
        if let Some((width, height, unit)) = by_box {
            payload["by_box"] = json!([width, height, unit.as_str()]);
        }
        if let Some(c) = count {
            payload["count"] = json!(c);
        }
        if let Some(s) = sort {
            payload["sort"] = json!(s);
        }

        let response = self
            .client
            .send_command("geospatial.geosearch", payload)
            .await?;
        let results: Vec<GeoradiusResult> =
            serde_json::from_value(response["results"].clone()).unwrap_or_default();
        Ok(results)
    }

    /// Get geohash strings for members (GEOHASH)
    ///
    /// # Arguments
    ///
    /// * `key` - Geospatial key
    /// * `members` - Array of member names
    ///
    /// # Returns
    ///
    /// Vector of geohash strings (None if member doesn't exist)
    pub async fn geohash(&self, key: &str, members: &[String]) -> Result<Vec<Option<String>>> {
        let payload = json!({
            "key": key,
            "members": members,
        });

        let response = self
            .client
            .send_command("geospatial.geohash", payload)
            .await?;
        let geohashes: Vec<Option<String>> =
            serde_json::from_value(response["geohashes"].clone()).unwrap_or_default();
        Ok(geohashes)
    }

    /// Retrieve geospatial statistics
    pub async fn stats(&self) -> Result<GeospatialStats> {
        let payload = json!({});
        let response = self
            .client
            .send_command("geospatial.stats", payload)
            .await?;
        let stats: GeospatialStats = serde_json::from_value(response).unwrap_or_default();
        Ok(stats)
    }
}
