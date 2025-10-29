# Tasks: Add Geospatial Indexes

> **Status**: 📋 Pending  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (7 commands, ~90 tasks, 4 weeks)

### Implementation
- [ ] Geospatial storage (backed by Sorted Set with geohash)
- [ ] GEOADD, GEODIST, GEORADIUS, GEORADIUSBYMEMBER, GEOPOS, GEOHASH, GEOSEARCH
- [ ] Haversine distance calculation
- [ ] Geohash encoding/decoding
- [ ] 15+ unit tests

### API
- [ ] 7 REST endpoints, 7 StreamableHTTP commands

### Testing
- [ ] 18+ unit tests, 12+ integration tests (accuracy, radius queries)

