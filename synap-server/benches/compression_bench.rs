//! Compression Benchmarks - LZ4 vs Zstd
//!
//! Compares compression ratio and speed for different algorithms
//! on typical Synap workloads (JSON, binary data, text)

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use synap_server::compression::{CompressionAlgorithm, Compressor};
use synap_server::compression::compressor::CompressionConfig;

/// Generate test data of various types
fn generate_test_data(size: usize, data_type: &str) -> Vec<u8> {
    match data_type {
        "json" => {
            // JSON-like data (typical for API responses)
            let json = serde_json::json!({
                "id": "user-12345",
                "name": "Andre Silva",
                "email": "andre@example.com",
                "age": 30,
                "active": true,
                "tags": ["developer", "rust", "tokio"],
                "metadata": {
                    "created_at": "2025-10-22T00:00:00Z",
                    "updated_at": "2025-10-22T00:00:00Z"
                }
            });
            let json_str = serde_json::to_string(&json).unwrap();
            json_str.as_bytes().repeat(size / json_str.len() + 1)[..size].to_vec()
        }
        "text" => {
            // Repetitive text (typical for logs)
            let text = "INFO: User logged in successfully. Session started. Token generated.\n";
            text.as_bytes().repeat(size / text.len() + 1)[..size].to_vec()
        }
        "binary" => {
            // Random binary data (worst case for compression)
            (0..size).map(|i| (i % 256) as u8).collect()
        }
        "sparse" => {
            // Sparse data (mostly zeros, best case)
            let mut data = vec![0u8; size];
            for i in (0..size).step_by(100) {
                data[i] = (i % 256) as u8;
            }
            data
        }
        _ => vec![0u8; size]}
}

/// Benchmark compression speed
fn bench_compression(c: &mut Criterion) {
    let compressor = Compressor::new(CompressionConfig::default());

    let data_types = vec!["json", "text", "binary", "sparse"];
    let sizes = vec![1024, 10 * 1024, 100 * 1024]; // 1KB, 10KB, 100KB

    for data_type in data_types {
        for size in &sizes {
            let data = generate_test_data(*size, data_type);

            let mut group = c.benchmark_group(format!("compress_{}", data_type));
            group.throughput(Throughput::Bytes(*size as u64));

            // LZ4 compression
            group.bench_with_input(BenchmarkId::new("LZ4", size), &data, |b, data| {
                b.iter(|| {
                    compressor
                        .compress(black_box(data), Some(CompressionAlgorithm::Lz4))
                        .unwrap()
                })
            });

            // Zstd compression (level 3)
            group.bench_with_input(BenchmarkId::new("Zstd", size), &data, |b, data| {
                b.iter(|| {
                    compressor
                        .compress(black_box(data), Some(CompressionAlgorithm::Zstd))
                        .unwrap()
                })
            });

            group.finish();
        }
    }
}

/// Benchmark decompression speed
fn bench_decompression(c: &mut Criterion) {
    let compressor = Compressor::new(CompressionConfig::default());

    let data_types = vec!["json", "text"];
    let sizes = vec![1024, 10 * 1024, 100 * 1024];

    for data_type in data_types {
        for size in &sizes {
            let data = generate_test_data(*size, data_type);

            // Pre-compress data
            let lz4_compressed = compressor
                .compress(&data, Some(CompressionAlgorithm::Lz4))
                .unwrap();
            let zstd_compressed = compressor
                .compress(&data, Some(CompressionAlgorithm::Zstd))
                .unwrap();

            let mut group = c.benchmark_group(format!("decompress_{}", data_type));
            group.throughput(Throughput::Bytes(*size as u64));

            // LZ4 decompression
            group.bench_with_input(
                BenchmarkId::new("LZ4", size),
                &lz4_compressed,
                |b, compressed| {
                    b.iter(|| {
                        compressor
                            .decompress(black_box(compressed), CompressionAlgorithm::Lz4)
                            .unwrap()
                    })
                },
            );

            // Zstd decompression
            group.bench_with_input(
                BenchmarkId::new("Zstd", size),
                &zstd_compressed,
                |b, compressed| {
                    b.iter(|| {
                        compressor
                            .decompress(black_box(compressed), CompressionAlgorithm::Zstd)
                            .unwrap()
                    })
                },
            );

            group.finish();
        }
    }
}

/// Benchmark compression ratio
fn bench_compression_ratio(c: &mut Criterion) {
    let compressor = Compressor::new(CompressionConfig::default());

    let mut group = c.benchmark_group("compression_ratio");

    let data_types = vec![
        ("json", generate_test_data(10240, "json")),
        ("text", generate_test_data(10240, "text")),
        ("binary", generate_test_data(10240, "binary")),
        ("sparse", generate_test_data(10240, "sparse")),
    ];

    for (name, data) in data_types {
        let original_size = data.len();

        // LZ4 ratio
        let lz4_compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Lz4))
            .unwrap();
        let lz4_ratio = original_size as f64 / lz4_compressed.len() as f64;

        // Zstd ratio
        let zstd_compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Zstd))
            .unwrap();
        let zstd_ratio = original_size as f64 / zstd_compressed.len() as f64;

        println!(
            "\n{} ({}KB): LZ4={:.2}x ({} bytes), Zstd={:.2}x ({} bytes)",
            name,
            original_size / 1024,
            lz4_ratio,
            lz4_compressed.len(),
            zstd_ratio,
            zstd_compressed.len()
        );

        group.bench_function(BenchmarkId::new("ratio_check", name), |b| {
            b.iter(|| {
                let _lz4 = compressor.compress(black_box(&data), Some(CompressionAlgorithm::Lz4));
                let _zstd = compressor.compress(black_box(&data), Some(CompressionAlgorithm::Zstd));
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compression,
    bench_decompression,
    bench_compression_ratio
);
criterion_main!(benches);
