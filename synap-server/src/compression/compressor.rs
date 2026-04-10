use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use tracing::debug;

/// Compression algorithm selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// LZ4 - Fast compression/decompression (default)
    #[default]
    Lz4,
    /// Zstandard - Better compression ratio
    Zstd,
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable compression globally
    pub enabled: bool,
    /// Minimum payload size to compress (bytes)
    pub min_payload_size: usize,
    /// Default algorithm
    pub default_algorithm: CompressionAlgorithm,
    /// Zstd compression level (1-22)
    pub zstd_level: i32,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_payload_size: 1024, // Don't compress < 1KB
            default_algorithm: CompressionAlgorithm::Lz4,
            zstd_level: 3, // Balanced compression
        }
    }
}

/// Main compressor interface
pub struct Compressor {
    config: CompressionConfig,
}

impl Compressor {
    /// Create new compressor with configuration
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Compress data using specified algorithm
    pub fn compress(
        &self,
        data: &[u8],
        algorithm: Option<CompressionAlgorithm>,
    ) -> Result<Vec<u8>, std::io::Error> {
        if !self.config.enabled || data.len() < self.config.min_payload_size {
            debug!("Skipping compression: size={} bytes", data.len());
            return Ok(data.to_vec());
        }

        let algo = algorithm.unwrap_or(self.config.default_algorithm);

        match algo {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Lz4 => self.compress_lz4(data),
            CompressionAlgorithm::Zstd => self.compress_zstd(data),
        }
    }

    /// Decompress data using specified algorithm
    pub fn decompress(
        &self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
    ) -> Result<Vec<u8>, std::io::Error> {
        match algorithm {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Lz4 => self.decompress_lz4(data),
            CompressionAlgorithm::Zstd => self.decompress_zstd(data),
        }
    }

    /// Compress using LZ4
    fn compress_lz4(&self, data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut encoder = lz4::EncoderBuilder::new()
            .level(4) // Fast compression
            .build(Vec::new())?;

        encoder.write_all(data)?;
        let (compressed, result) = encoder.finish();
        result?;

        let ratio = data.len() as f64 / compressed.len() as f64;
        debug!(
            "LZ4 compressed: {} → {} bytes (ratio: {:.2}x)",
            data.len(),
            compressed.len(),
            ratio
        );

        Ok(compressed)
    }

    /// Decompress using LZ4
    fn decompress_lz4(&self, data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut decoder = lz4::Decoder::new(data)?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        debug!(
            "LZ4 decompressed: {} → {} bytes",
            data.len(),
            decompressed.len()
        );
        Ok(decompressed)
    }

    /// Compress using Zstd
    fn compress_zstd(&self, data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let compressed = zstd::encode_all(data, self.config.zstd_level)?;

        let ratio = data.len() as f64 / compressed.len() as f64;
        debug!(
            "Zstd compressed: {} → {} bytes (ratio: {:.2}x)",
            data.len(),
            compressed.len(),
            ratio
        );

        Ok(compressed)
    }

    /// Decompress using Zstd
    fn decompress_zstd(&self, data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let decompressed = zstd::decode_all(data)?;

        debug!(
            "Zstd decompressed: {} → {} bytes",
            data.len(),
            decompressed.len()
        );
        Ok(decompressed)
    }

    /// Calculate compression ratio
    pub fn compression_ratio(&self, original: usize, compressed: usize) -> f64 {
        if compressed == 0 {
            return 1.0;
        }
        original as f64 / compressed as f64
    }

    /// Estimate if compression would be beneficial
    pub fn should_compress(&self, data: &[u8]) -> bool {
        self.config.enabled && data.len() >= self.config.min_payload_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_compression() {
        let config = CompressionConfig {
            enabled: true,
            min_payload_size: 10,
            ..Default::default()
        };
        let compressor = Compressor::new(config);

        let data = b"Hello, World! This is a test string that should compress well.".repeat(10);
        let compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Lz4))
            .unwrap();

        assert!(compressed.len() < data.len());

        let decompressed = compressor
            .decompress(&compressed, CompressionAlgorithm::Lz4)
            .unwrap();
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_zstd_compression() {
        let config = CompressionConfig {
            enabled: true,
            min_payload_size: 10,
            default_algorithm: CompressionAlgorithm::Zstd,
            ..Default::default()
        };
        let compressor = Compressor::new(config);

        let data = b"Hello, World! This is a test string that should compress well.".repeat(10);
        let compressed = compressor
            .compress(&data, Some(CompressionAlgorithm::Zstd))
            .unwrap();

        assert!(compressed.len() < data.len());

        let decompressed = compressor
            .decompress(&compressed, CompressionAlgorithm::Zstd)
            .unwrap();
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_skip_small_payloads() {
        let config = CompressionConfig {
            enabled: true,
            min_payload_size: 1024,
            ..Default::default()
        };
        let compressor = Compressor::new(config);

        let small_data = b"Small";
        let result = compressor.compress(small_data, None).unwrap();

        // Should not compress (too small)
        assert_eq!(result, small_data);
    }

    #[test]
    fn test_compression_disabled() {
        let config = CompressionConfig {
            enabled: false,
            ..Default::default()
        };
        let compressor = Compressor::new(config);

        let data = b"Hello, World!".repeat(100);
        let result = compressor.compress(&data, None).unwrap();

        // Should return original data
        assert_eq!(result, data);
    }

    #[test]
    fn test_compression_ratio() {
        let compressor = Compressor::new(CompressionConfig::default());

        let ratio = compressor.compression_ratio(1000, 500);
        assert_eq!(ratio, 2.0);

        let ratio = compressor.compression_ratio(1000, 333);
        assert!((ratio - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_should_compress() {
        let config = CompressionConfig {
            enabled: true,
            min_payload_size: 1024,
            ..Default::default()
        };
        let compressor = Compressor::new(config);

        let large_data = vec![0u8; 2048];
        assert!(compressor.should_compress(&large_data));

        let small_data = vec![0u8; 512];
        assert!(!compressor.should_compress(&small_data));
    }
}
