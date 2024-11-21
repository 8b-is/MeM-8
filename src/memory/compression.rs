use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    LZ4,
    // Future: Add Zstandard, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionMetrics {
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_time: Duration,
    pub algorithm: CompressionAlgorithm,
}

impl CompressionMetrics {
    pub fn compression_ratio(&self) -> f32 {
        if self.original_size == 0 {
            return 1.0;
        }
        self.compressed_size as f32 / self.original_size as f32
    }
}

pub struct Compressor {
    algorithm: CompressionAlgorithm,
}

impl Compressor {
    pub fn new(algorithm: CompressionAlgorithm) -> Self {
        Self { algorithm }
    }

    pub fn compress(&self, data: &[u8]) -> (Vec<u8>, CompressionMetrics) {
        let start = std::time::Instant::now();
        let original_size = data.len();

        let (compressed_data, compressed_size) = match self.algorithm {
            CompressionAlgorithm::None => (data.to_vec(), data.len()),
            CompressionAlgorithm::LZ4 => {
                let compressed = compress_prepend_size(data);
                (compressed.clone(), compressed.len())
            }
        };

        let metrics = CompressionMetrics {
            original_size,
            compressed_size,
            compression_time: start.elapsed(),
            algorithm: self.algorithm,
        };

        (compressed_data, metrics)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        match self.algorithm {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::LZ4 => decompress_size_prepended(data)
                .map_err(|e| format!("LZ4 decompression error: {}", e)),
        }
    }
} 