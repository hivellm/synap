# Add Sorted Set Data Structure

> **Status**: Draft  
> **Priority**: High (Phase 2)  
> **Target**: v0.6.0-alpha  
> **Duration**: 6 weeks

## Why

Sorted Sets enable leaderboards, priority queues, time-series indexing, and range queries on scored data. Essential for gaming platforms, rate limiting, auto-complete, and temporal data.

## What Changes

Implement Redis-compatible Sorted Set with 25+ commands using dual data structure (HashMap + BTreeMap):

**Commands**: ZADD, ZREM, ZSCORE, ZRANK, ZRANGE, ZREVRANGE, ZRANGEBYSCORE, ZCOUNT, ZPOPMIN, ZPOPMAX, ZINCRBY, ZINTERSTORE, ZUNIONSTORE, etc.

**API**: REST (25 endpoints) + StreamableHTTP (25 commands) + MCP (6 tools)

## Impact

**NEW**: `synap-server/src/core/sorted_set.rs` (~1200 lines)  
**Complexity**: High (dual data structure, weighted set operations)  
**Performance**: ZADD <200µs, ZRANGE <1ms (100 items)

