# Tasks: Add Geospatial Indexes

> **Status**: 🚧 In Progress (core ops shipped, GEOSEARCH + unit tests pending)  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (7 commands, ~90 tasks, 4 weeks)

### Implementation
- [x] Geospatial storage (Sorted Set + geohash backing)
- [x] GEOADD, GEODIST, GEORADIUS, GEORADIUSBYMEMBER, GEOPOS, GEOHASH
- [ ] GEOSEARCH command
- [x] Haversine distance calculation
- [x] Geohash encoding/decoding
- [ ] 15+ unit tests (currently 0)

### API
- [x] 6 REST endpoints, 6 StreamableHTTP commands (GEOADD/GEODIST/GEORADIUS/GEORADIUSBYMEMBER/GEOPOS/GEOHASH + stats)
- [ ] REST + StreamableHTTP coverage for GEOSEARCH

### Testing
- [ ] 18+ unit tests (currently 0)
- [x] 12+ integration tests (15 implemented)

