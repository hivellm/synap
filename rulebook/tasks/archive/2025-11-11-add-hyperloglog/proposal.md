# Add HyperLogLog Support

> **Status**: Draft  
> **Priority**: Low (Phase 4)  
> **Target**: v0.8.0+  
> **Duration**: 2 weeks

## Why

Probabilistic cardinality estimation for counting unique items with ~0.81% error in only 12KB memory.

## What Changes

Implement Redis HyperLogLog:

**Commands**: PFADD, PFCOUNT, PFMERGE

**Use Cases**: Unique visitor counts, distinct IP tracking, cardinality estimation

## Impact

**NEW**: `synap-server/src/core/hyperloglog.rs` (~300 lines)  
**Complexity**: Medium (use `hyperloglog` crate)  
**Storage**: 12KB fixed per HLL

