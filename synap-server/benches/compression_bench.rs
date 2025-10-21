use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use synap_server::compression::{CompressionAlgorithm, Compressor};

/// Generate test data with different characteristics
fn generate_test_data(size: usize, compressibility: &str) -> Vec<u8> {
    match compressibility {
        "high" => {
            // Highly compressible (repeated patterns)
            vec![b'A'; size]
        }
        "medium" => {
            // Medium compressibility (some repetition)
            let pattern = b"Hello, World! This is a test message. ";
            pattern.iter().cycle().take(size).copied().collect()
        }
        "low" => {
            // Low compressibility (random-like data)
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut data = Vec::with_capacity(size);
            for i in 0..size {
                let mut hasher = DefaultHasher::new();
                i.hash(&mut hasher);
                data.push((hasher.finish() % 256) as u8);
            }
            data
        }
        "json" => {
            // Realistic JSON data
            let json = r#"{"user_id": 12345, "name": "John Doe", "email": "john@example.com", "tags": ["premium", "verified"], "metadata": {"last_login": "2025-10-21T10:30:00Z", "ip": "192.168.1.1"}}"#;
            json.as_bytes().iter().cycle().take(size).copied().collect()
        }
        _ => vec![0u8; size],
    }
}

/// Benchmark: LZ4 compression across different data sizes
fn bench_lz4_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("lz4_compress");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("highly_compressible", size),
            size,
            |b, &s| {
                let config = synap_server::compression::compressor::CompressionConfig {
                    enabled: true,
                    min_payload_size: 0,
                    default_algorithm: CompressionAlgorithm::Lz4,
                    zstd_level: 3,
                };
                let compressor = Compressor::new(config);
                let data = generate_test_data(s, "high");

                b.iter(|| {
                    compressor
                        .compress(black_box(&data), Some(CompressionAlgorithm::Lz4))
                        .unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("medium_compressible", size),
            size,
            |b, &s| {
                let config = synap_server::compression::compressor::CompressionConfig {
                    enabled: true,
                    min_payload_size: 0,
                    default_algorithm: CompressionAlgorithm::Lz4,
                    zstd_level: 3,
                };
                let compressor = Compressor::new(config);
                let data = generate_test_data(s, "medium");

                b.iter(|| {
                    compressor
                        .compress(black_box(&data), Some(CompressionAlgorithm::Lz4))
                        .unwrap()
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("json_data", size), size, |b, &s| {
            let config = synap_server::compression::compressor::CompressionConfig {
                enabled: true,
                min_payload_size: 0,
                default_algorithm: CompressionAlgorithm::Lz4,
                zstd_level: 3,
            };
            let compressor = Compressor::new(config);
            let data = generate_test_data(s, "json");

            b.iter(|| {
                compressor
                    .compress(black_box(&data), Some(CompressionAlgorithm::Lz4))
                    .unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark: Zstd compression across different compression levels
fn bench_zstd_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("zstd_compress");

    let data_size = 16384;
    let data = generate_test_data(data_size, "medium");

    for level in [1, 3, 6, 9].iter() {
        group.throughput(Throughput::Bytes(data_size as u64));

        group.bench_with_input(
            BenchmarkId::new("compression_level", level),
            level,
            |b, &lvl| {
                let config = synap_server::compression::compressor::CompressionConfig {
                    enabled: true,
                    min_payload_size: 0,
                    default_algorithm: CompressionAlgorithm::Zstd,
                    zstd_level: lvl,
                };
                let compressor = Compressor::new(config);

                b.iter(|| {
                    compressor
                        .compress(black_box(&data), Some(CompressionAlgorithm::Zstd))
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: LZ4 decompression
fn bench_lz4_decompress(c: &mut Criterion) {
    let mut group = c.benchmark_group("lz4_decompress");

    for size in [1024, 4096, 16384, 65536].iter() {
        let config = synap_server::compression::compressor::CompressionConfig {
            enabled: true,
            min_payload_size: 0,
            default_algorithm: CompressionAlgorithm::Lz4,
            zstd_level: 3,
        };
        let compressor = Compressor::new(config);
        let data = generate_test_data(*size, "medium");
        let compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Lz4))
            .unwrap();

        group.throughput(Throughput::Bytes(compressed.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("size", size),
            &compressed,
            |b, comp_data| {
                b.iter(|| {
                    compressor
                        .decompress(black_box(comp_data), CompressionAlgorithm::Lz4)
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Zstd decompression
fn bench_zstd_decompress(c: &mut Criterion) {
    let mut group = c.benchmark_group("zstd_decompress");

    let data_size = 16384;
    let data = generate_test_data(data_size, "medium");

    for level in [1, 3, 6, 9].iter() {
        let config = synap_server::compression::compressor::CompressionConfig {
            enabled: true,
            min_payload_size: 0,
            default_algorithm: CompressionAlgorithm::Zstd,
            zstd_level: *level,
        };
        let compressor = Compressor::new(config);
        let compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Zstd))
            .unwrap();

        group.throughput(Throughput::Bytes(compressed.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("level", level),
            &compressed,
            |b, comp_data| {
                b.iter(|| {
                    compressor
                        .decompress(black_box(comp_data), CompressionAlgorithm::Zstd)
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Compression ratio analysis
fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratio");

    let size = 65536;

    for data_type in ["high", "medium", "low", "json"].iter() {
        let data = generate_test_data(size, data_type);

        group.bench_with_input(BenchmarkId::new("lz4", data_type), &data, |b, d| {
            let config = synap_server::compression::compressor::CompressionConfig {
                enabled: true,
                min_payload_size: 0,
                default_algorithm: CompressionAlgorithm::Lz4,
                zstd_level: 3,
            };
            let compressor = Compressor::new(config);

            b.iter(|| {
                let compressed = compressor
                    .compress(black_box(d), Some(CompressionAlgorithm::Lz4))
                    .unwrap();
                let ratio = compressor.compression_ratio(d.len(), compressed.len());
                black_box(ratio);
            });
        });

        group.bench_with_input(BenchmarkId::new("zstd", data_type), &data, |b, d| {
            let config = synap_server::compression::compressor::CompressionConfig {
                enabled: true,
                min_payload_size: 0,
                default_algorithm: CompressionAlgorithm::Zstd,
                zstd_level: 3,
            };
            let compressor = Compressor::new(config);

            b.iter(|| {
                let compressed = compressor
                    .compress(black_box(d), Some(CompressionAlgorithm::Zstd))
                    .unwrap();
                let ratio = compressor.compression_ratio(d.len(), compressed.len());
                black_box(ratio);
            });
        });
    }

    group.finish();
}

/// Benchmark: Round-trip (compress + decompress)
fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_roundtrip");

    for size in [1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("lz4", size), size, |b, &s| {
            let config = synap_server::compression::compressor::CompressionConfig {
                enabled: true,
                min_payload_size: 0,
                default_algorithm: CompressionAlgorithm::Lz4,
                zstd_level: 3,
            };
            let compressor = Compressor::new(config);
            let data = generate_test_data(s, "medium");

            b.iter(|| {
                let compressed = compressor
                    .compress(black_box(&data), Some(CompressionAlgorithm::Lz4))
                    .unwrap();
                let decompressed = compressor
                    .decompress(&compressed, CompressionAlgorithm::Lz4)
                    .unwrap();
                black_box(decompressed);
            });
        });

        group.bench_with_input(BenchmarkId::new("zstd", size), size, |b, &s| {
            let config = synap_server::compression::compressor::CompressionConfig {
                enabled: true,
                min_payload_size: 0,
                default_algorithm: CompressionAlgorithm::Zstd,
                zstd_level: 3,
            };
            let compressor = Compressor::new(config);
            let data = generate_test_data(s, "medium");

            b.iter(|| {
                let compressed = compressor
                    .compress(black_box(&data), Some(CompressionAlgorithm::Zstd))
                    .unwrap();
                let decompressed = compressor
                    .decompress(&compressed, CompressionAlgorithm::Zstd)
                    .unwrap();
                black_box(decompressed);
            });
        });
    }

    group.finish();
}

/// Benchmark: Should compress decision
fn bench_should_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("should_compress");

    group.bench_function("enabled_above_threshold", |b| {
        let config = synap_server::compression::compressor::CompressionConfig {
            enabled: true,
            min_payload_size: 1024,
            default_algorithm: CompressionAlgorithm::Lz4,
            zstd_level: 3,
        };
        let compressor = Compressor::new(config);
        let data = vec![0u8; 2048];

        b.iter(|| {
            let should = compressor.should_compress(black_box(&data));
            black_box(should);
        });
    });

    group.bench_function("enabled_below_threshold", |b| {
        let config = synap_server::compression::compressor::CompressionConfig {
            enabled: true,
            min_payload_size: 1024,
            default_algorithm: CompressionAlgorithm::Lz4,
            zstd_level: 3,
        };
        let compressor = Compressor::new(config);
        let data = vec![0u8; 512];

        b.iter(|| {
            let should = compressor.should_compress(black_box(&data));
            black_box(should);
        });
    });

    group.bench_function("disabled", |b| {
        let config = synap_server::compression::compressor::CompressionConfig {
            enabled: false,
            min_payload_size: 1024,
            default_algorithm: CompressionAlgorithm::Lz4,
            zstd_level: 3,
        };
        let compressor = Compressor::new(config);
        let data = vec![0u8; 2048];

        b.iter(|| {
            let should = compressor.should_compress(black_box(&data));
            black_box(should);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lz4_compress,
    bench_zstd_compress,
    bench_lz4_decompress,
    bench_zstd_decompress,
    bench_compression_ratio,
    bench_roundtrip,
    bench_should_compress
);

criterion_main!(benches);
