## 1. Infrastructure: simd Module + Cargo Feature
- [ ] 1.1 Add `simd` feature to `synap-server/Cargo.toml` (default-features includes it); add `memchr` dependency
- [ ] 1.2 Create `synap-server/src/simd/mod.rs` with public dispatch API: `popcount_slice(bytes)`, `bitop_and(a, b)`, `bitop_or(a, b)`, `bitop_xor(a, b)`, `bitop_not(a)`, `bitpos_set(bytes)`, `bitpos_clear(bytes)`, `max_reduce_u8(a, b)`
- [ ] 1.3 Create `synap-server/src/simd/fallback.rs` — portable u64-chunked implementations for all functions (no unsafe, pure safe Rust)
- [ ] 1.4 Create `synap-server/src/simd/x86.rs` — SSE2 baseline + AVX2 fast path; runtime dispatch via `is_x86_feature_detected!("avx2")` / `is_x86_feature_detected!("sse2")`; every unsafe block has `// SAFETY:` comment
- [ ] 1.5 Create `synap-server/src/simd/aarch64.rs` — NEON implementation using `std::arch::aarch64::*`; guarded by `#[cfg(target_arch = "aarch64")]`
- [ ] 1.6 Create `synap-server/src/simd/wasm.rs` — SIMD128 implementation using `std::arch::wasm32::*`; guarded by `#[cfg(target_arch = "wasm32")]`
- [ ] 1.7 Wire dispatch in `simd/mod.rs`: `#[cfg(target_arch = "x86_64")]` -> x86, `#[cfg(target_arch = "aarch64")]` -> aarch64, `#[cfg(target_arch = "wasm32")]` -> wasm, fallback for all others
- [ ] 1.8 Register `pub mod simd;` in `synap-server/src/lib.rs`

## 2. Bitmap Acceleration (core/bitmap.rs)
- [ ] 2.1 Replace `bitcount()` scalar loop with `simd::popcount_slice(&self.data[range])`
- [ ] 2.2 Replace `bitop_and()` loop with `simd::bitop_and(a, b)` — returns new `Vec<u8>`
- [ ] 2.3 Replace `bitop_or()` loop with `simd::bitop_or(a, b)`
- [ ] 2.4 Replace `bitop_xor()` loop with `simd::bitop_xor(a, b)`
- [ ] 2.5 Replace `bitop_not()` loop with `simd::bitop_not(a)`
- [ ] 2.6 Replace `bitpos()` byte scan with `simd::bitpos_set()` / `simd::bitpos_clear()`

## 3. HyperLogLog Acceleration (core/hyperloglog.rs)
- [ ] 3.1 Replace `pfcount()` register popcount loop with `simd::popcount_slice(&self.registers)`
- [ ] 3.2 Replace `pfmerge()` element-wise max loop with `simd::max_reduce_u8(dest, src)`

## 4. KEYS/SCAN Acceleration (core/kv_store.rs)
- [ ] 4.1 Add `memchr` to `scan()` prefix matching: use `memchr::memmem::Finder` for substring search instead of `str::contains()`
- [ ] 4.2 Add `memchr::memchr` for single-byte prefix checks in pattern scanning hot path

## 5. Set Operations Acceleration (core/set.rs)
- [ ] 5.1 In `sinter()` (intersection), use `memchr`-based membership probe for large sets
- [ ] 5.2 In `sunion()`, detect when one set is much smaller — iterate small set, probe large set via SIMD byte scan
- [ ] 5.3 In `sdiff()`, same probe strategy for the difference loop

## 6. Benchmarks
- [ ] 6.1 Add `benches/simd_bench.rs` with criterion benchmarks: `bitcount/1KB`, `bitcount/1MB`, `bitop_and/512KB`, `pfcount`, `pfmerge`, `scan_prefix/10k_keys`
- [ ] 6.2 Run benchmarks with `--features simd` vs `--no-default-features` (fallback path); record results

## 7. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 7.1 Write correctness tests: for each SIMD function, compare SIMD output vs fallback output on randomized input (proptest or manual random seeds)
- [ ] 7.2 Write `#[cfg(target_arch = "x86_64")]` tests that explicitly exercise SSE2 and AVX2 paths using `#[target_feature(enable = "avx2")]`
- [ ] 7.3 CI: confirm `cargo test --no-default-features` passes (fallback-only path compiles and is correct)
- [ ] 7.4 CI: confirm `cargo test --all-features` passes
- [ ] 7.5 Update or create documentation covering the implementation
- [ ] 7.6 Write tests covering the new behavior
- [ ] 7.7 Run tests and confirm they pass
