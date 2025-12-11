# Add Geospatial Indexes

> **Status**: Draft  
> **Priority**: Low (Phase 4)  
> **Target**: v0.8.0+  
> **Duration**: 4 weeks

## Why

Location-based queries for store finders, ride-sharing, delivery zones, and proximity search.

## What Changes

Implement Redis geospatial operations:

**Commands**: GEOADD, GEODIST, GEORADIUS, GEORADIUSBYMEMBER, GEOPOS, GEOHASH, GEOSEARCH

**Features**:
- Internally uses Sorted Sets with geohash scores
- Haversine distance calculation
- Radius queries

**API**: REST (7 endpoints) + StreamableHTTP (7 commands)

## Impact

**NEW**: `synap-server/src/core/geospatial.rs` (~600 lines)  
**Complexity**: High (geospatial math, geohash encoding)  
**Dependencies**: `geo` or `geohash` crate

