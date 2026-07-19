# Tasks: Add Geospatial Indexes

> **Status**: ✅ Complete  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (7 commands, ~90 tasks, 4 weeks)

### Implementation
- [x] Geospatial storage (Sorted Set + geohash backing)
- [x] GEOADD, GEODIST, GEORADIUS, GEORADIUSBYMEMBER, GEOPOS, GEOHASH
- [x] GEOSEARCH command (FROMMEMBER/FROMLONLAT + BYRADIUS/BYBOX)
- [x] Haversine distance calculation
- [x] Geohash encoding/decoding
- [x] 23 unit tests (comprehensive coverage)

### API
- [x] 8 REST endpoints (GEOADD/GEODIST/GEORADIUS/GEORADIUSBYMEMBER/GEOPOS/GEOHASH/GEOSEARCH/STATS)
- [x] 8 StreamableHTTP commands (geospatial.geoadd/geodist/georadius/georadiusbymember/geopos/geohash/geosearch/stats)

### Testing
- [x] 23 unit tests (comprehensive coverage: GEOADD, GEODIST, GEOPOS, GEOHASH, GEORADIUS, GEORADIUSBYMEMBER, GEOSEARCH, stats)
- [x] 17 integration tests (REST + StreamableHTTP, including GEOSEARCH)
- [x] SDK S2S tests:
  - Python: 12 tests
  - TypeScript: 11 tests (5 GEOSEARCH)
  - Rust: 8 tests (3 GEOSEARCH)
  - PHP: 9 tests (3 GEOSEARCH)
  - C#: 9 tests (3 GEOSEARCH)

### SDK Support
- [x] Python SDK - GeospatialManager with GEOSEARCH support
- [x] TypeScript SDK - GeospatialManager with GEOSEARCH support
- [x] Rust SDK - GeospatialManager with GEOSEARCH support
- [x] PHP SDK - GeospatialManager with GEOSEARCH support
- [x] C# SDK - GeospatialManager with GEOSEARCH support

### Documentation
- [x] OpenAPI specification updated with all geospatial endpoints
- [x] CHANGELOG.md updated

