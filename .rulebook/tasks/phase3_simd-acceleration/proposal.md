# Proposal: SIMD Acceleration — All Platforms, All Applicable Operations

## Why

Synap processes bulk data operations (BITCOUNT, BITOP, PFCOUNT, PFMERGE, KEYS/SCAN pattern
matching, set intersection/union) using scalar byte-by-byte loops. Modern CPUs can process
16-32 bytes per clock cycle using SIMD registers (SSE2, AVX2 on x86_64; NEON on aarch64;
SIMD128 on wasm32), providing 4-32x throughput improvement on these operations at zero
algorithmic cost.

Critical operations affected:
- BITCOUNT on a 1MB bitmap: ~1M iterations scalar vs ~32K iterations with AVX2
- BITOP AND/OR/XOR on two 512KB bitmaps: same improvement ratio
- PFCOUNT: popcount over 16KB register array — pure POPCNT
- PFMERGE: 16KB max-reduce over two register arrays — fully vectorizable
- KEYS/SCAN: scanning thousands of key strings for prefix/suffix match
- Set SINTER/SUNION/SDIFF: membership testing over large sets

The implementation uses a layered strategy:
1. Portable safe fallback: u64::count_ones() chunks (compiles to POPCNT on any target),
   chunk-based OR/AND/XOR (auto-vectorized by LLVM in release builds)
2. Explicit SIMD via std::arch intrinsics with #[target_feature(enable = "...")] for
   x86_64 (SSE2, AVX2), aarch64 (NEON), and wasm32 (simd128) gated by runtime detection
3. memchr crate for key scanning — battle-tested cross-platform SIMD

All SIMD paths are behind a simd Cargo feature (default on) and fall back gracefully when
the CPU does not support the required instruction set. Every unsafe block has a SAFETY comment.

Source: docs/analysis/synap-vs-redis/ (Phase 3 optimization targets; execution-plan Phase 3.1)

## What Changes

- ADDED: synap-server/src/simd/ module with platform-specific implementations
  - mod.rs: public API, runtime dispatch, simd feature gate
  - x86.rs: SSE2 + AVX2 implementations (cfg target_arch = x86_64)
  - aarch64.rs: NEON implementations (cfg target_arch = aarch64)
  - wasm.rs: SIMD128 implementations (cfg target_arch = wasm32)
  - fallback.rs: portable u64-chunked implementations (all platforms)
- MODIFIED: core/bitmap.rs — bitcount(), bitop_and/or/xor/not(), bitpos() use simd:: dispatch
- MODIFIED: core/hyperloglog.rs — pfcount() and pfmerge() use simd::popcount_slice() and simd::max_reduce_u8()
- MODIFIED: core/kv_store.rs scan() — use memchr crate for prefix/byte pattern acceleration
- MODIFIED: core/set.rs — sinter(), sunion(), sdiff() use SIMD-accelerated membership check
- MODIFIED: Cargo.toml — add memchr dependency, add simd Cargo feature (default = ["simd"])

## Impact

- Affected specs: specs/simd/spec.md
- Affected code: synap-server/src/core/bitmap.rs, hyperloglog.rs, kv_store.rs, set.rs; new synap-server/src/simd/
- Breaking change: NO
- User benefit: BITCOUNT/BITOP 4-32x faster; PFCOUNT/PFMERGE 4-8x faster; KEYS/SCAN 2-4x faster on large keyspaces; zero regression on CPUs without SIMD support
