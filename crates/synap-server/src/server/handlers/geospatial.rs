use super::*;

#[derive(Debug, Serialize)]
pub struct GeospatialDistResponse {
    pub key: String,
    pub distance: Option<f64>,
    pub unit: String,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialRadiusRequest {
    pub center_lat: f64,
    pub center_lon: f64,
    pub radius: f64,
    #[serde(default = "default_unit")]
    pub unit: String,
    #[serde(default)]
    pub with_dist: bool,
    #[serde(default)]
    pub with_coord: bool,
    pub count: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GeospatialRadiusResponse {
    pub key: String,
    pub results: Vec<GeospatialRadiusResult>,
}

#[derive(Debug, Serialize)]
pub struct GeospatialRadiusResult {
    pub member: String,
    pub distance: Option<f64>,
    pub coord: Option<GeospatialCoord>,
}

#[derive(Debug, Serialize)]
pub struct GeospatialCoord {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialRadiusByMemberRequest {
    pub member: String,
    pub radius: f64,
    #[serde(default = "default_unit")]
    pub unit: String,
    #[serde(default)]
    pub with_dist: bool,
    #[serde(default)]
    pub with_coord: bool,
    pub count: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialPosRequest {
    pub members: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GeospatialPosResponse {
    pub key: String,
    pub coordinates: Vec<Option<GeospatialCoord>>,
}

#[derive(Debug, Serialize)]
pub struct GeospatialHashResponse {
    pub key: String,
    pub geohashes: Vec<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialSearchRequest {
    pub from_member: Option<String>,
    pub from_lonlat: Option<(f64, f64)>,
    pub by_radius: Option<(f64, String)>,
    pub by_box: Option<(f64, f64, String)>,
    pub with_dist: Option<bool>,
    pub with_coord: Option<bool>,
    pub with_hash: Option<bool>,
    pub count: Option<usize>,
    pub sort: Option<String>,
}

// ==================== Geospatial REST Handlers ====================

/// POST /geospatial/:key/geoadd - Add geospatial locations
pub async fn geospatial_geoadd(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<GeospatialAddRequest>,
) -> Result<Json<GeospatialAddResponse>, SynapError> {
    debug!(
        "REST GEOADD key={} locations={} nx={} xx={} ch={}",
        key,
        req.locations.len(),
        req.nx,
        req.xx,
        req.ch
    );

    let locations: Vec<(f64, f64, Vec<u8>)> = req
        .locations
        .into_iter()
        .map(|loc| (loc.lat, loc.lon, loc.member.into_bytes()))
        .collect();

    let added = state
        .geospatial_store
        .geoadd(&key, locations, req.nx, req.xx, req.ch)?;

    Ok(Json(GeospatialAddResponse { key, added }))
}

/// GET /geospatial/:key/geodist/:member1/:member2 - Calculate distance
pub async fn geospatial_geodist(
    State(state): State<AppState>,
    Path((key, member1, member2)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GeospatialDistResponse>, SynapError> {
    let unit_str = params
        .get("unit")
        .cloned()
        .unwrap_or_else(|| "m".to_string());
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;

    debug!(
        "REST GEODIST key={} member1={} member2={} unit={:?}",
        key, member1, member2, unit
    );

    let distance =
        state
            .geospatial_store
            .geodist(&key, member1.as_bytes(), member2.as_bytes(), unit)?;

    Ok(Json(GeospatialDistResponse {
        key,
        distance,
        unit: unit_str,
    }))
}

/// GET /geospatial/:key/georadius - Query within radius
pub async fn geospatial_georadius(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GeospatialRadiusResponse>, SynapError> {
    let center_lat: f64 = params
        .get("lat")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'lat' parameter".to_string()))?;
    let center_lon: f64 = params
        .get("lon")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'lon' parameter".to_string()))?;
    let radius: f64 = params
        .get("radius")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'radius' parameter".to_string()))?;
    let unit_str = params
        .get("unit")
        .cloned()
        .unwrap_or_else(|| "m".to_string());
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;
    let with_dist = params.get("withdist").map(|s| s == "true").unwrap_or(false);
    let with_coord = params
        .get("withcoord")
        .map(|s| s == "true")
        .unwrap_or(false);
    let count = params.get("count").and_then(|s| s.parse().ok());
    let sort = params.get("sort").cloned();

    debug!(
        "REST GEORADIUS key={} lat={} lon={} radius={} unit={:?}",
        key, center_lat, center_lon, radius, unit
    );

    let results = state.geospatial_store.georadius(
        &key,
        center_lat,
        center_lon,
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    )?;

    let response_results: Vec<GeospatialRadiusResult> = results
        .into_iter()
        .map(|(member, distance, coord)| {
            let member_str = String::from_utf8_lossy(&member).to_string();
            let coord_opt = coord.map(|c| GeospatialCoord {
                lat: c.lat,
                lon: c.lon,
            });
            GeospatialRadiusResult {
                member: member_str,
                distance,
                coord: coord_opt,
            }
        })
        .collect();

    Ok(Json(GeospatialRadiusResponse {
        key,
        results: response_results,
    }))
}

/// GET /geospatial/:key/georadiusbymember/:member - Query within radius of member
pub async fn geospatial_georadiusbymember(
    State(state): State<AppState>,
    Path((key, member)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GeospatialRadiusResponse>, SynapError> {
    let radius: f64 = params
        .get("radius")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'radius' parameter".to_string()))?;
    let unit_str = params
        .get("unit")
        .cloned()
        .unwrap_or_else(|| "m".to_string());
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;
    let with_dist = params.get("withdist").map(|s| s == "true").unwrap_or(false);
    let with_coord = params
        .get("withcoord")
        .map(|s| s == "true")
        .unwrap_or(false);
    let count = params.get("count").and_then(|s| s.parse().ok());
    let sort = params.get("sort").cloned();

    debug!(
        "REST GEORADIUSBYMEMBER key={} member={} radius={} unit={:?}",
        key, member, radius, unit
    );

    let results = state.geospatial_store.georadiusbymember(
        &key,
        member.as_bytes(),
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    )?;

    let response_results: Vec<GeospatialRadiusResult> = results
        .into_iter()
        .map(|(member_bytes, distance, coord)| {
            let member_str = String::from_utf8_lossy(&member_bytes).to_string();
            let coord_opt = coord.map(|c| GeospatialCoord {
                lat: c.lat,
                lon: c.lon,
            });
            GeospatialRadiusResult {
                member: member_str,
                distance,
                coord: coord_opt,
            }
        })
        .collect();

    Ok(Json(GeospatialRadiusResponse {
        key,
        results: response_results,
    }))
}

/// POST /geospatial/:key/geopos - Get coordinates of members
pub async fn geospatial_geopos(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<GeospatialPosRequest>,
) -> Result<Json<GeospatialPosResponse>, SynapError> {
    debug!("REST GEOPOS key={} members={:?}", key, req.members);

    let members: Vec<Vec<u8>> = req.members.into_iter().map(|m| m.into_bytes()).collect();

    let coordinates = state.geospatial_store.geopos(&key, &members)?;

    let response_coords: Vec<Option<GeospatialCoord>> = coordinates
        .into_iter()
        .map(|coord_opt| {
            coord_opt.map(|c| GeospatialCoord {
                lat: c.lat,
                lon: c.lon,
            })
        })
        .collect();

    Ok(Json(GeospatialPosResponse {
        key,
        coordinates: response_coords,
    }))
}

/// POST /geospatial/:key/geohash - Get geohash strings
pub async fn geospatial_geohash(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<GeospatialPosRequest>,
) -> Result<Json<GeospatialHashResponse>, SynapError> {
    debug!("REST GEOHASH key={} members={:?}", key, req.members);

    let members: Vec<Vec<u8>> = req.members.into_iter().map(|m| m.into_bytes()).collect();

    let geohashes = state.geospatial_store.geohash(&key, &members)?;

    Ok(Json(GeospatialHashResponse { key, geohashes }))
}

/// POST /geospatial/:key/geosearch - Advanced geospatial search
pub async fn geospatial_geosearch(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<GeospatialSearchRequest>,
) -> Result<Json<GeospatialRadiusResponse>, SynapError> {
    debug!(
        "REST GEOSEARCH key={} from_member={:?} from_lonlat={:?}",
        key, req.from_member, req.from_lonlat
    );

    let from_member = req.from_member.as_ref().map(|s| s.as_bytes());
    let from_lonlat = req.from_lonlat;

    let by_radius = req.by_radius.map(|(r, u)| {
        let unit = u
            .parse::<crate::core::DistanceUnit>()
            .unwrap_or(crate::core::DistanceUnit::Meters);
        (r, unit)
    });

    let by_box = req.by_box.map(|(w, h, u)| {
        let unit = u
            .parse::<crate::core::DistanceUnit>()
            .unwrap_or(crate::core::DistanceUnit::Meters);
        (w, h, unit)
    });

    let results = state.geospatial_store.geosearch(
        &key,
        from_member,
        from_lonlat,
        by_radius,
        by_box,
        req.with_dist.unwrap_or(false),
        req.with_coord.unwrap_or(false),
        req.with_hash.unwrap_or(false),
        req.count,
        req.sort.as_deref(),
    )?;

    let response_results: Vec<GeospatialRadiusResult> = results
        .into_iter()
        .map(|(member_bytes, distance, coord)| {
            let member_str = String::from_utf8_lossy(&member_bytes).to_string();
            let coord_opt = coord.map(|c| GeospatialCoord {
                lat: c.lat,
                lon: c.lon,
            });
            GeospatialRadiusResult {
                member: member_str,
                distance,
                coord: coord_opt,
            }
        })
        .collect();

    Ok(Json(GeospatialRadiusResponse {
        key,
        results: response_results,
    }))
}

/// GET /geospatial/stats - Retrieve geospatial statistics
pub async fn geospatial_stats(
    State(state): State<AppState>,
) -> Result<Json<crate::core::GeospatialStats>, SynapError> {
    debug!("REST GEOSPATIAL STATS");

    let stats = state.geospatial_store.stats();

    Ok(Json(stats))
}

// ==================== Geospatial StreamableHTTP Command Handlers ====================

pub(super) async fn handle_geospatial_geoadd_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let locations_array = request
        .payload
        .get("locations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'locations' array".to_string()))?;

    let mut locations = Vec::new();
    for loc in locations_array {
        let lat = loc
            .get("lat")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| SynapError::InvalidRequest("Location missing 'lat'".to_string()))?;
        let lon = loc
            .get("lon")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| SynapError::InvalidRequest("Location missing 'lon'".to_string()))?;
        let member = loc
            .get("member")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapError::InvalidRequest("Location missing 'member'".to_string()))?;

        locations.push((lat, lon, member.as_bytes().to_vec()));
    }

    let nx = request
        .payload
        .get("nx")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let xx = request
        .payload
        .get("xx")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ch = request
        .payload
        .get("ch")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let added = state.geospatial_store.geoadd(key, locations, nx, xx, ch)?;

    Ok(serde_json::json!({ "key": key, "added": added }))
}

pub(super) async fn handle_geospatial_geodist_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let member1 = request
        .payload
        .get("member1")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member1' field".to_string()))?;
    let member2 = request
        .payload
        .get("member2")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member2' field".to_string()))?;

    let unit_str = request
        .payload
        .get("unit")
        .and_then(|v| v.as_str())
        .unwrap_or("m");
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;

    let distance =
        state
            .geospatial_store
            .geodist(key, member1.as_bytes(), member2.as_bytes(), unit)?;

    Ok(serde_json::json!({
        "key": key,
        "member1": member1,
        "member2": member2,
        "distance": distance,
        "unit": unit_str
    }))
}

pub(super) async fn handle_geospatial_georadius_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let center_lat = request
        .payload
        .get("center_lat")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'center_lat' field".to_string()))?;
    let center_lon = request
        .payload
        .get("center_lon")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'center_lon' field".to_string()))?;
    let radius = request
        .payload
        .get("radius")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'radius' field".to_string()))?;

    let unit_str = request
        .payload
        .get("unit")
        .and_then(|v| v.as_str())
        .unwrap_or("m");
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;

    let with_dist = request
        .payload
        .get("with_dist")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let with_coord = request
        .payload
        .get("with_coord")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let sort = request
        .payload
        .get("sort")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let results = state.geospatial_store.georadius(
        key,
        center_lat,
        center_lon,
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    )?;

    let json_results: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(member, distance, coord)| {
            let mut obj = serde_json::json!({
                "member": String::from_utf8_lossy(&member).to_string(),
            });
            if let Some(dist) = distance {
                obj["distance"] = serde_json::json!(dist);
            }
            if let Some(coord_val) = coord {
                obj["coord"] = serde_json::json!({
                    "lat": coord_val.lat,
                    "lon": coord_val.lon,
                });
            }
            obj
        })
        .collect();

    Ok(serde_json::json!({ "key": key, "results": json_results }))
}

pub(super) async fn handle_geospatial_georadiusbymember_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let member = request
        .payload
        .get("member")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let radius = request
        .payload
        .get("radius")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'radius' field".to_string()))?;

    let unit_str = request
        .payload
        .get("unit")
        .and_then(|v| v.as_str())
        .unwrap_or("m");
    let unit = unit_str.parse::<crate::core::DistanceUnit>()?;

    let with_dist = request
        .payload
        .get("with_dist")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let with_coord = request
        .payload
        .get("with_coord")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let sort = request
        .payload
        .get("sort")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let results = state.geospatial_store.georadiusbymember(
        key,
        member.as_bytes(),
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    )?;

    let json_results: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(member_bytes, distance, coord)| {
            let mut obj = serde_json::json!({
                "member": String::from_utf8_lossy(&member_bytes).to_string(),
            });
            if let Some(dist) = distance {
                obj["distance"] = serde_json::json!(dist);
            }
            if let Some(coord_val) = coord {
                obj["coord"] = serde_json::json!({
                    "lat": coord_val.lat,
                    "lon": coord_val.lon,
                });
            }
            obj
        })
        .collect();

    Ok(serde_json::json!({ "key": key, "results": json_results }))
}

pub(super) async fn handle_geospatial_geopos_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members_array = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?;

    let members: Vec<Vec<u8>> = members_array
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.as_bytes().to_vec()))
        .collect();

    let coordinates = state.geospatial_store.geopos(key, &members)?;

    let json_coords: Vec<Option<serde_json::Value>> = coordinates
        .into_iter()
        .map(|coord_opt| {
            coord_opt.map(|c| {
                serde_json::json!({
                    "lat": c.lat,
                    "lon": c.lon,
                })
            })
        })
        .collect();

    Ok(serde_json::json!({
        "key": key,
        "coordinates": json_coords
    }))
}

pub(super) async fn handle_geospatial_geohash_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members_array = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?;

    let members: Vec<Vec<u8>> = members_array
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.as_bytes().to_vec()))
        .collect();

    let geohashes = state.geospatial_store.geohash(key, &members)?;

    Ok(serde_json::json!({
        "key": key,
        "geohashes": geohashes
    }))
}

pub(super) async fn handle_geospatial_geosearch_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let from_member = request
        .payload
        .get("from_member")
        .and_then(|v| v.as_str())
        .map(|s| s.as_bytes());
    let from_lonlat = request.payload.get("from_lonlat").and_then(|v| {
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 {
                let lon = arr[0].as_f64()?;
                let lat = arr[1].as_f64()?;
                Some((lon, lat))
            } else {
                None
            }
        } else {
            None
        }
    });

    let by_radius = request.payload.get("by_radius").and_then(|v| {
        if let Some(arr) = v.as_array() {
            if arr.len() == 2 {
                let radius = arr[0].as_f64()?;
                let unit_str = arr[1].as_str()?;
                let unit = unit_str.parse::<crate::core::DistanceUnit>().ok()?;
                Some((radius, unit))
            } else {
                None
            }
        } else {
            None
        }
    });

    let by_box = request.payload.get("by_box").and_then(|v| {
        if let Some(arr) = v.as_array() {
            if arr.len() == 3 {
                let width = arr[0].as_f64()?;
                let height = arr[1].as_f64()?;
                let unit_str = arr[2].as_str()?;
                let unit = unit_str.parse::<crate::core::DistanceUnit>().ok()?;
                Some((width, height, unit))
            } else {
                None
            }
        } else {
            None
        }
    });

    let with_dist = request
        .payload
        .get("with_dist")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let with_coord = request
        .payload
        .get("with_coord")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let with_hash = request
        .payload
        .get("with_hash")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let sort = request
        .payload
        .get("sort")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let results = state.geospatial_store.geosearch(
        key,
        from_member,
        from_lonlat,
        by_radius,
        by_box,
        with_dist,
        with_coord,
        with_hash,
        count,
        sort.as_deref(),
    )?;

    let json_results: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(member_bytes, distance, coord)| {
            let mut result = serde_json::json!({
                "member": String::from_utf8_lossy(&member_bytes),
            });
            if let Some(dist) = distance {
                result["distance"] = serde_json::json!(dist);
            }
            if let Some(c) = coord {
                result["coord"] = serde_json::json!({
                    "lat": c.lat,
                    "lon": c.lon,
                });
            }
            result
        })
        .collect();

    Ok(serde_json::json!({
        "key": key,
        "results": json_results
    }))
}

pub(super) async fn handle_geospatial_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.geospatial_store.stats();

    Ok(serde_json::json!({
        "total_keys": stats.total_keys,
        "total_locations": stats.total_locations,
        "geoadd_count": stats.geoadd_count,
        "geodist_count": stats.geodist_count,
        "georadius_count": stats.georadius_count,
        "geopos_count": stats.geopos_count,
        "geohash_count": stats.geohash_count,
    }))
}

// ============================================================================
// Cluster Management Handlers
// ============================================================================
