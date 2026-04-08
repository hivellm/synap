//! Benchmarks for the SIMD primitives in `synap_server::simd`.
//!
//! Run with `cargo bench --bench simd_bench`.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rand::{Rng, SeedableRng, rngs::StdRng};
use synap_server::simd;

fn random_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..len).map(|_| rng.random::<u8>()).collect()
}

fn bench_popcount(c: &mut Criterion) {
    let mut group = c.benchmark_group("popcount");
    for &size in &[1024usize, 1024 * 64, 1024 * 1024] {
        let data = random_bytes(1, size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("simd", size), &data, |b, d| {
            b.iter(|| simd::popcount_slice(d))
        });
        group.bench_with_input(BenchmarkId::new("fallback", size), &data, |b, d| {
            b.iter(|| simd::fallback::popcount_slice(d))
        });
    }
    group.finish();
}

fn bench_bitop_and(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitop_and");
    for &size in &[1024usize * 64, 1024 * 512, 1024 * 1024] {
        let a = random_bytes(2, size);
        let b = random_bytes(3, size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &(a.clone(), b.clone()),
            |bn, (x, y)| bn.iter(|| simd::bitop_and(x, y)),
        );
        group.bench_with_input(BenchmarkId::new("fallback", size), &(a, b), |bn, (x, y)| {
            bn.iter(|| simd::fallback::bitop_and(x, y))
        });
    }
    group.finish();
}

fn bench_max_reduce(c: &mut Criterion) {
    let mut group = c.benchmark_group("max_reduce_u8");
    let size = 16 * 1024; // HLL register count
    let src = random_bytes(4, size);
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("simd", |b| {
        b.iter_batched(
            || random_bytes(5, size),
            |mut dest| simd::max_reduce_u8(&mut dest, &src),
            criterion::BatchSize::SmallInput,
        )
    });
    group.bench_function("fallback", |b| {
        b.iter_batched(
            || random_bytes(5, size),
            |mut dest| simd::fallback::max_reduce_u8(&mut dest, &src),
            criterion::BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(benches, bench_popcount, bench_bitop_and, bench_max_reduce);
criterion_main!(benches);
