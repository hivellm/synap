# Tasks: Add Bitmap Operations

> **Status**: ✅ Complete  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (8 commands, ~70 tasks, 3 weeks)

### Implementation
- [x] Bitmap storage (Vec<u8>)
- [x] SETBIT, GETBIT, BITCOUNT, BITPOS, BITOP, BITFIELD
- [x] 30 unit tests (comprehensive coverage: GET/SET/INCRBY, overflow handling, signed/unsigned, edge cases)

### API
- [x] 8 REST endpoints (SETBIT, GETBIT, BITCOUNT, BITPOS, BITOP, BITFIELD, STATS)
- [x] 8 StreamableHTTP commands (bitmap.setbit, bitmap.getbit, bitmap.bitcount, bitmap.bitpos, bitmap.bitop, bitmap.bitfield, bitmap.stats)

### Testing
- [x] 30 unit tests covering:
  - Basic operations (SETBIT/GETBIT, BITCOUNT, BITPOS, BITOP)
  - BITFIELD GET/SET/INCRBY operations
  - Overflow handling (WRAP, SAT, FAIL) for signed/unsigned
  - Different bit widths (4, 8, 12, 16, 24, 32, 64 bits)
  - Signed/unsigned interop
  - Cross-byte boundaries
  - Large offsets
  - Overlapping fields
  - Edge cases (empty bitmaps, invalid widths, partial reads)
- [x] 12+ integration tests (REST API endpoints tested)

### Notes
- BITFIELD supports GET, SET, INCRBY operations
- Overflow handling: WRAP (default), SAT (saturate), FAIL (error on overflow)
- Little-endian bit order for BITFIELD (consistent with Redis)
- All tests passing ✅
