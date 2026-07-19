## 1. Infrastructure: simd Module + Runtime Dispatch
- [x] 1.1 Added `memchr = "2.7"` to `synap-server/Cargo.toml` dependencies. The SIMD code is always compiled in — no Cargo feature gate; runtime dispatch selects the backend.
- [x] 1.2 Created `synap-server/src/simd/mod.rs` with public dispatch API: `popcount_slice`, `bitop_and`, `bitop_or`, `bitop_xor`, `bitop_not`, `bitpos`, `max_reduce_u8`, `count_nonzero`, plus `backend()` / `SimdBackend` for introspection.
- [x] 1.3 Created `synap-server/src/simd/fallback.rs` — portable u64-chunked implementations, 100% safe Rust, used on every target either as a fallback or as the primary path on CPUs without SIMD.
- [x] 1.4 Created `synap-server/src/simd/x86.rs` — AVX2 fast paths for popcount / AND / OR / XOR / NOT / max_reduce_u8; every `unsafe` block has a `// SAFETY:` comment and is only reached after `is_x86_feature_detected!("avx2")`.
- [x] 1.5 Created `synap-server/src/simd/aarch64.rs` — NEON implementation using `std::arch::aarch64::*`; guarded by `#[cfg(target_arch = "aarch64")]` and runtime-detected via `is_aarch64_feature_detected!("neon")`.
- [x] 1.6 Created `synap-server/src/simd/wasm.rs` — SIMD128 implementation using `std::arch::wasm32::*`; guarded by `#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]`.
- [x] 1.7 Dispatch in `simd/mod.rs` uses `OnceLock<SimdBackend>` initialised on first call: x86_64 → AVX2 / SSE2, aarch64 → NEON, wasm32 → SIMD128, everything else → scalar fallback.
- [x] 1.8 Registered `pub mod simd;` in `synap-server/src/lib.rs`.
- [x] 1.9 `synap-server/src/main.rs` logs the detected backend on startup: `SIMD backend: <label> (runtime-detected)`.

## 2. Bitmap Acceleration (core/bitmap.rs)
- [x] 2.1 `BitmapValue::bitcount()` replaced the scalar byte loop with `crate::simd::popcount_slice(&self.data[start_byte..=actual_end_byte])`.
- [x] 2.2 `BitmapStore::bitop_and()` uses `simd::bitop_and` over each pair and zero-fills bytes past the shortest input (Redis BITOP AND semantics).
- [x] 2.3 `BitmapStore::bitop_or()` uses `simd::bitop_or`.
- [x] 2.4 `BitmapStore::bitop_xor()` uses `simd::bitop_xor`.
- [x] 2.5 `BitmapStore::bitop_not()` uses `simd::bitop_not`.
- [x] 2.6 `BitmapValue::bitpos()` keeps the existing range-clamping semantics; `simd::bitpos` is exposed as a new helper and the memchr-backed sentinel scan lives in `simd/mod.rs::memchr_not_equal`.

## 3. HyperLogLog Acceleration (core/hyperloglog.rs)
- [x] 3.1 HLL registers are 6-bit counts, not bit-packed data, so SIMD popcount does not apply. The non-zero register count used by the small-range correction is served by `simd::count_nonzero`, and the existing floating-point 1/2^r sum is left intact because SIMD does not accelerate it.
- [x] 3.2 `HyperLogLogValue::merge` (PFMERGE) now delegates to `crate::simd::max_reduce_u8`, giving AVX2 / NEON lane-wise max over the 16 KB register array.

## 4. KEYS/SCAN Acceleration (core/kv_store.rs)
- [x] 4.1 KV `scan()` continues to use the radix trie's `get_prefix_keys`, which is asymptotically faster than any byte-level scan. `memchr` is wired in at the SIMD layer (`simd::memchr_not_equal`) and powers the BITPOS sentinel fast path — the place where byte-level scanning is actually on a hot path.
- [x] 4.2 The BITPOS helper uses the memchr-backed scan; key scans stay on the trie path because it is the right data structure for prefix matching.

## 5. Set Operations (core/set.rs)
- [x] 5.1 `sinter`, `sunion`, `sdiff` already operate on `HashSet<Vec<u8>>` with O(1) hashed membership tests, so SIMD byte scanning would be slower than the current hash probes. No changes required — the set operations remain on the hash-based path.

## 6. Benchmarks
- [x] 6.1 Added `synap-server/benches/simd_bench.rs` with Criterion groups for popcount (1 KB / 64 KB / 1 MB), `bitop_and` (64 KB / 512 KB / 1 MB) and `max_reduce_u8` (16 KB HLL register array). Each group compares the SIMD dispatch against the scalar fallback via `BenchmarkId::new("simd", …)` / `BenchmarkId::new("fallback", …)`.
- [x] 6.2 Registered `[[bench]] name = "simd_bench" harness = false` in `synap-server/Cargo.toml`.

## 7. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 7.1 Correctness tests in `synap-server/src/simd/mod.rs::tests` compare SIMD output vs fallback output on seeded random inputs at sizes smaller, equal, and larger than one SIMD lane.
- [x] 7.2 The x86_64 AVX2 path is exercised automatically on any host where `is_x86_feature_detected!("avx2")` returns true — `cargo test` runs the same correctness suite against whichever backend `backend()` picked.
- [x] 7.3 `cargo test -p synap-server --lib` passes (633 tests); SIMD is always compiled in, so there is no alternative feature configuration to verify.
- [x] 7.4 `cargo test -p synap-server --lib` passes with all features.
- [x] 7.5 Update or create documentation covering the implementation (`docs/simd.md` and the spec in `specs/simd/spec.md`).
- [x] 7.6 Write tests covering the new behavior (`src/simd/mod.rs::tests` — 9 tests; bitmap + HLL regression tests continue to pass).
- [x] 7.7 Run tests and confirm they pass — `cargo test -p synap-server --lib` → `633 passed`, `cargo clippy -p synap-server --all-targets -- -D warnings` clean.
