# Add Bitmap Operations

> **Status**: Draft  
> **Priority**: Low (Phase 4)  
> **Target**: v0.8.0+  
> **Duration**: 3 weeks

## Why

Bit-level operations for activity tracking, feature flags, and efficient boolean storage.

## What Changes

Implement Redis bitmap operations:

**Commands**: SETBIT, GETBIT, BITCOUNT, BITOP (AND/OR/XOR/NOT), BITPOS, BITFIELD

**API**: REST (8 endpoints) + StreamableHTTP (8 commands)

## Impact

**NEW**: `synap-server/src/core/bitmap.rs` (~500 lines)  
**Complexity**: Low-Medium (bit manipulation)  
**Storage**: Vec<u8> or BitVec

