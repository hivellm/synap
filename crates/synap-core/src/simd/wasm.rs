//! wasm32 SIMD128 implementations. Only compiled on `target_arch = "wasm32"`
//! with the `simd128` target feature enabled at compile time.

#![cfg(all(target_arch = "wasm32", target_feature = "simd128"))]

use std::arch::wasm32::*;

#[inline]
pub fn popcount_simd128(bytes: &[u8]) -> u64 {
    let mut acc: u64 = 0;
    let chunks = bytes.chunks_exact(16);
    let tail = chunks.remainder();
    for chunk in chunks {
        // SAFETY: chunk is exactly 16 bytes.
        let v = unsafe { v128_load(chunk.as_ptr() as *const v128) };
        let cnt = u8x16_popcnt(v);
        let mut tmp = [0u8; 16];
        // SAFETY: tmp is 16 bytes.
        unsafe { v128_store(tmp.as_mut_ptr() as *mut v128, cnt) };
        acc += tmp.iter().map(|&b| b as u64).sum::<u64>();
    }
    acc + super::fallback::popcount_slice(tail)
}

#[inline]
pub fn bitop_and_simd128(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { v128_load(a.as_ptr().add(i) as *const v128) };
        let vb = unsafe { v128_load(b.as_ptr().add(i) as *const v128) };
        let r = v128_and(va, vb);
        unsafe { v128_store(out.as_mut_ptr().add(i) as *mut v128, r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] & b[i];
        i += 1;
    }
}

#[inline]
pub fn bitop_or_simd128(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { v128_load(a.as_ptr().add(i) as *const v128) };
        let vb = unsafe { v128_load(b.as_ptr().add(i) as *const v128) };
        let r = v128_or(va, vb);
        unsafe { v128_store(out.as_mut_ptr().add(i) as *mut v128, r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] | b[i];
        i += 1;
    }
}

#[inline]
pub fn bitop_xor_simd128(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { v128_load(a.as_ptr().add(i) as *const v128) };
        let vb = unsafe { v128_load(b.as_ptr().add(i) as *const v128) };
        let r = v128_xor(va, vb);
        unsafe { v128_store(out.as_mut_ptr().add(i) as *mut v128, r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] ^ b[i];
        i += 1;
    }
}

#[inline]
pub fn bitop_not_simd128(a: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { v128_load(a.as_ptr().add(i) as *const v128) };
        let r = v128_not(va);
        unsafe { v128_store(out.as_mut_ptr().add(i) as *mut v128, r) };
        i += 16;
    }
    while i < len {
        out[i] = !a[i];
        i += 1;
    }
}

#[inline]
pub fn max_reduce_u8_simd128(dest: &mut [u8], src: &[u8]) {
    let len = dest.len().min(src.len());
    let mut i = 0;
    while i + 16 <= len {
        let vd = unsafe { v128_load(dest.as_ptr().add(i) as *const v128) };
        let vs = unsafe { v128_load(src.as_ptr().add(i) as *const v128) };
        let r = u8x16_max(vd, vs);
        unsafe { v128_store(dest.as_mut_ptr().add(i) as *mut v128, r) };
        i += 16;
    }
    while i < len {
        if src[i] > dest[i] {
            dest[i] = src[i];
        }
        i += 1;
    }
}
