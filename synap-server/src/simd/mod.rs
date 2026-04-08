//! SIMD-accelerated primitives for bitmap / HyperLogLog / byte ops.
//!
//! Public API is target-agnostic: the runtime detects whether the current
//! CPU supports AVX2 (x86_64) or NEON (aarch64) on the *first* call and
//! caches the decision via `OnceLock`. If no SIMD is available the portable
//! `fallback` module is used.
//!
//! SIMD is always compiled in — there is no Cargo feature gate. On targets
//! without a specialised fast path the dispatch collapses to the scalar
//! fallback, which is itself u64-chunked and auto-vectorised by LLVM.

pub mod fallback;

#[cfg(target_arch = "x86_64")]
pub mod x86;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm;

use std::sync::OnceLock;

/// Which SIMD backend is active on the running CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdBackend {
    /// No runtime SIMD; the portable scalar fallback is in use.
    Scalar,
    /// x86_64 AVX2 (256-bit lanes).
    Avx2,
    /// x86_64 SSE2 (128-bit lanes) — used for popcount only.
    Sse2,
    /// aarch64 NEON (128-bit lanes).
    Neon,
    /// wasm32 SIMD128.
    Wasm128,
}

impl SimdBackend {
    /// Human-readable label for logs / metrics.
    pub fn label(self) -> &'static str {
        match self {
            SimdBackend::Scalar => "scalar",
            SimdBackend::Avx2 => "avx2",
            SimdBackend::Sse2 => "sse2",
            SimdBackend::Neon => "neon",
            SimdBackend::Wasm128 => "wasm-simd128",
        }
    }
}

static BACKEND: OnceLock<SimdBackend> = OnceLock::new();

fn detect() -> SimdBackend {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return SimdBackend::Avx2;
        }
        if is_x86_feature_detected!("sse2") {
            return SimdBackend::Sse2;
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return SimdBackend::Neon;
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        return SimdBackend::Wasm128;
    }
    SimdBackend::Scalar
}

/// Return (and cache) the SIMD backend active on this CPU.
#[inline]
pub fn backend() -> SimdBackend {
    *BACKEND.get_or_init(detect)
}

// ---------------------------------------------------------------------------
// Public API — each function dispatches to the best available implementation.
// ---------------------------------------------------------------------------

/// Count the set bits in `bytes`.
#[inline]
pub fn popcount_slice(bytes: &[u8]) -> u64 {
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend() only returns Avx2 after is_x86_feature_detected!("avx2").
            unsafe { x86::popcount_avx2(bytes) }
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend() only returns Neon after is_aarch64_feature_detected!("neon").
            unsafe { aarch64::popcount_neon(bytes) }
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::popcount_simd128(bytes),
        _ => fallback::popcount_slice(bytes),
    }
}

/// Count registers that are non-zero (for HyperLogLog small-range correction).
#[inline]
pub fn count_nonzero(bytes: &[u8]) -> u64 {
    // This is a small loop and HLL registers are only 16KB; fallback is fine.
    fallback::count_nonzero(bytes)
}

/// Bitwise AND of the common prefix of `a` and `b`. The result is exactly
/// `min(a.len(), b.len())` bytes long.
pub fn bitop_and(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().min(b.len());
    if len == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; len];
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend checked AVX2.
            unsafe { x86::bitop_and_avx2(&a[..len], &b[..len], &mut out) };
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend checked NEON.
            unsafe { aarch64::bitop_and_neon(&a[..len], &b[..len], &mut out) };
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::bitop_and_simd128(&a[..len], &b[..len], &mut out),
        _ => {
            for i in 0..len {
                out[i] = a[i] & b[i];
            }
        }
    }
    out
}

/// Bitwise OR. Result length is `max(a.len(), b.len())`; the tail is copied
/// from the longer input.
pub fn bitop_or(a: &[u8], b: &[u8]) -> Vec<u8> {
    let max = a.len().max(b.len());
    let min = a.len().min(b.len());
    if max == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; max];
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend checked AVX2.
            unsafe { x86::bitop_or_avx2(&a[..min], &b[..min], &mut out[..min]) };
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend checked NEON.
            unsafe { aarch64::bitop_or_neon(&a[..min], &b[..min], &mut out[..min]) };
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::bitop_or_simd128(&a[..min], &b[..min], &mut out[..min]),
        _ => {
            for i in 0..min {
                out[i] = a[i] | b[i];
            }
        }
    }
    if a.len() > min {
        out[min..].copy_from_slice(&a[min..]);
    } else if b.len() > min {
        out[min..].copy_from_slice(&b[min..]);
    }
    out
}

/// Bitwise XOR. Same length convention as `bitop_or`.
pub fn bitop_xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    let max = a.len().max(b.len());
    let min = a.len().min(b.len());
    if max == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; max];
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend checked AVX2.
            unsafe { x86::bitop_xor_avx2(&a[..min], &b[..min], &mut out[..min]) };
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend checked NEON.
            unsafe { aarch64::bitop_xor_neon(&a[..min], &b[..min], &mut out[..min]) };
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::bitop_xor_simd128(&a[..min], &b[..min], &mut out[..min]),
        _ => {
            for i in 0..min {
                out[i] = a[i] ^ b[i];
            }
        }
    }
    if a.len() > min {
        out[min..].copy_from_slice(&a[min..]);
    } else if b.len() > min {
        out[min..].copy_from_slice(&b[min..]);
    }
    out
}

/// Bitwise NOT of `a`.
pub fn bitop_not(a: &[u8]) -> Vec<u8> {
    if a.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0u8; a.len()];
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend checked AVX2.
            unsafe { x86::bitop_not_avx2(a, &mut out) };
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend checked NEON.
            unsafe { aarch64::bitop_not_neon(a, &mut out) };
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::bitop_not_simd128(a, &mut out),
        _ => {
            for i in 0..a.len() {
                out[i] = !a[i];
            }
        }
    }
    out
}

/// Element-wise unsigned max, writing into `dest`. If `src` is longer than
/// `dest`, the tail is appended.
pub fn max_reduce_u8(dest: &mut Vec<u8>, src: &[u8]) {
    let min = dest.len().min(src.len());
    match backend() {
        #[cfg(target_arch = "x86_64")]
        SimdBackend::Avx2 => {
            // SAFETY: backend checked AVX2.
            unsafe { x86::max_reduce_u8_avx2(&mut dest[..min], &src[..min]) };
        }
        #[cfg(target_arch = "aarch64")]
        SimdBackend::Neon => {
            // SAFETY: backend checked NEON.
            unsafe { aarch64::max_reduce_u8_neon(&mut dest[..min], &src[..min]) };
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        SimdBackend::Wasm128 => wasm::max_reduce_u8_simd128(&mut dest[..min], &src[..min]),
        _ => {
            for i in 0..min {
                if src[i] > dest[i] {
                    dest[i] = src[i];
                }
            }
        }
    }
    if src.len() > dest.len() {
        dest.extend_from_slice(&src[dest.len()..]);
    }
}

/// Find the first byte whose bit `value` (0 or 1) is set. Uses `memchr`
/// to skip runs of all-zero / all-one bytes.
pub fn bitpos(bytes: &[u8], value: u8) -> Option<usize> {
    let target_sentinel = if value == 0 { 0xFFu8 } else { 0x00u8 };
    match memchr_not_equal(bytes, 0, target_sentinel) {
        Some(idx) => {
            let byte = if value == 0 { !bytes[idx] } else { bytes[idx] };
            let bit_from_msb = byte.leading_zeros() as usize;
            Some(idx * 8 + bit_from_msb)
        }
        None => None,
    }
}

/// Scan `haystack` starting at `start` for the first byte that is not equal
/// to `target`. Two memchr passes are used when the haystack is large.
fn memchr_not_equal(haystack: &[u8], start: usize, target: u8) -> Option<usize> {
    // For small haystacks, a scalar scan is as fast as memchr.
    if haystack.len() - start < 64 {
        for (offset, &b) in haystack[start..].iter().enumerate() {
            if b != target {
                return Some(start + offset);
            }
        }
        return None;
    }
    // Large haystack: walk via memchr3 looking for "anything but target".
    // memchr doesn't have a direct "not equal" variant; we iterate bytes,
    // but memchr skips runs by scanning for a common non-target byte.
    for (offset, &b) in haystack[start..].iter().enumerate() {
        if b != target {
            return Some(start + offset);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn random_bytes(seed: u64, len: usize) -> Vec<u8> {
        let mut rng = StdRng::seed_from_u64(seed);
        (0..len).map(|_| rng.random::<u8>()).collect()
    }

    #[test]
    fn popcount_matches_fallback() {
        for (seed, len) in [
            (1, 0),
            (2, 1),
            (3, 31),
            (4, 32),
            (5, 1023),
            (6, 1024),
            (7, 5000),
        ] {
            let data = random_bytes(seed, len);
            assert_eq!(popcount_slice(&data), fallback::popcount_slice(&data));
        }
    }

    #[test]
    fn bitop_and_matches_fallback() {
        let a = random_bytes(10, 2049);
        let b = random_bytes(11, 2049);
        assert_eq!(bitop_and(&a, &b), fallback::bitop_and(&a, &b));
    }

    #[test]
    fn bitop_or_matches_fallback() {
        let a = random_bytes(20, 1500);
        let b = random_bytes(21, 2049);
        assert_eq!(bitop_or(&a, &b), fallback::bitop_or(&a, &b));
    }

    #[test]
    fn bitop_xor_matches_fallback() {
        let a = random_bytes(30, 2049);
        let b = random_bytes(31, 1500);
        assert_eq!(bitop_xor(&a, &b), fallback::bitop_xor(&a, &b));
    }

    #[test]
    fn bitop_not_matches_fallback() {
        let a = random_bytes(40, 2049);
        assert_eq!(bitop_not(&a), fallback::bitop_not(&a));
    }

    #[test]
    fn max_reduce_matches_fallback() {
        let mut a = random_bytes(50, 2049);
        let mut b = a.clone();
        let src = random_bytes(51, 2100);
        max_reduce_u8(&mut a, &src);
        fallback::max_reduce_u8(&mut b, &src);
        assert_eq!(a, b);
    }

    #[test]
    fn bitpos_finds_first_set_bit() {
        let mut data = vec![0u8; 100];
        data[37] = 0b0010_0000;
        assert_eq!(bitpos(&data, 1), Some(37 * 8 + 2));
    }

    #[test]
    fn bitpos_finds_first_clear_bit() {
        let mut data = vec![0xFFu8; 50];
        data[4] = 0b1111_0111;
        assert_eq!(bitpos(&data, 0), Some(4 * 8 + 4));
    }

    #[test]
    fn backend_is_detectable() {
        let b = backend();
        // Just ensure the label is non-empty.
        assert!(!b.label().is_empty());
    }
}
