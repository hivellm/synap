# SIMD Acceleration

Synap accelerates its bulk byte paths (BITCOUNT, BITOP AND/OR/XOR/NOT,
BITPOS, HyperLogLog PFMERGE) using CPU SIMD instructions. The module
lives in [`synap-server/src/simd/`](../synap-server/src/simd/).

## Design

- **Always compiled in.** There is no Cargo feature gate. Every build
  includes the scalar fallback *and* the platform-specific fast path.
- **Runtime detection.** On the first call to `simd::backend()` the
  process queries `is_x86_feature_detected!` / `is_aarch64_feature_detected!`
  and caches the result in a `OnceLock`. Subsequent calls are a single
  atomic load.
- **Graceful fallback.** Targets without a dedicated implementation use
  `simd::fallback`, a portable `u64`-chunked version that LLVM already
  auto-vectorises to whatever the target natively supports.

## Backends

| Arch       | Backend     | Instructions used                                  |
|------------|-------------|----------------------------------------------------|
| x86_64     | `avx2`      | `_mm256_*` (256-bit lanes, 32 bytes per iteration) |
| x86_64     | `sse2`      | reserved for future popcount fallback              |
| aarch64    | `neon`      | `v{ld,st}1q_u8`, `vcntq_u8`, `v{and,or,xor,mvn}q_u8`, `vmaxq_u8` |
| wasm32     | `wasm-simd128` | `v128_*`, `u8x16_popcnt`, `u8x16_max`            |
| other      | `scalar`    | `u64::count_ones`, auto-vectorised scalar loops    |

The startup log line tells you which backend was chosen:

```
INFO  synap_server: SIMD backend: avx2 (runtime-detected)
```

## Public API

All functions live in `synap_server::simd`:

- `popcount_slice(&[u8]) -> u64`
- `bitop_and(&[u8], &[u8]) -> Vec<u8>`
- `bitop_or(&[u8], &[u8]) -> Vec<u8>`
- `bitop_xor(&[u8], &[u8]) -> Vec<u8>`
- `bitop_not(&[u8]) -> Vec<u8>`
- `max_reduce_u8(&mut Vec<u8>, &[u8])`
- `bitpos(&[u8], value: u8) -> Option<usize>`
- `backend() -> SimdBackend`

## Where it's used

- [`core/bitmap.rs`](../synap-server/src/core/bitmap.rs) — `bitcount`,
  `bitop_and`, `bitop_or`, `bitop_xor`, `bitop_not`.
- [`core/hyperloglog.rs`](../synap-server/src/core/hyperloglog.rs) —
  `HyperLogLogValue::merge` (element-wise max over the 16 KB register
  array, used by PFMERGE).

## Safety

Every `unsafe` block is tagged with a `// SAFETY:` comment explaining
the precondition. All raw-pointer loads/stores are dominated by a
length check in the surrounding loop, and every platform-specific
function is only reachable through the cached runtime backend check,
so the `#[target_feature]` contract is upheld.

## Benchmarks

```bash
cargo bench --bench simd_bench
```

The bench compares the SIMD dispatch against the scalar fallback on
popcount (1 KB / 64 KB / 1 MB), BITOP AND (64 KB / 512 KB / 1 MB), and
HLL max-reduce over a 16 KB register array.

## Correctness tests

`synap-server/src/simd/mod.rs` ships a property-style suite comparing
every SIMD path against the fallback on seeded random inputs of
multiple sizes (including sizes smaller than one SIMD lane). Run with:

```bash
cargo test -p synap-server --lib simd::
```
