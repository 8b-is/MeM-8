use reed_solomon_erasure::galois_8::ReedSolomon;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCorrectionMetrics {
    pub original_size: usize,
    pub parity_size: usize,
    pub corrections_performed: u32,
    pub last_correction_time: Option<std::time::SystemTime>,
}

pub struct ReedSolomonEC {
    rs: ReedSolomon,
    data_shards: usize,
    parity_shards: usize,
}

impl ReedSolomonEC {
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self, String> {
        let rs = ReedSolomon::new(data_shards, parity_shards)
            .map_err(|e| format!("Failed to create Reed-Solomon: {}", e))?;
        
        Ok(Self {
            rs,
            data_shards,
            parity_shards,
        })
    }

    pub fn encode(&self, data: &[u8]) -> Result<(Vec<Vec<u8>>, ErrorCorrectionMetrics), String> {
        let start = Instant::now();
        
        // Split data into shards
        let shard_size = (data.len() + self.data_shards - 1) / self.data_shards;
        let mut shards = vec![vec![0u8; shard_size]; self.data_shards + self.parity_shards];
        
        // Fill data shards
        for (i, chunk) in data.chunks(shard_size).enumerate() {
            shards[i][..chunk.len()].copy_from_slice(chunk);
        }
        
        // Generate parity shards
        self.rs.encode(&mut shards)
            .map_err(|e| format!("Encoding failed: {}", e))?;
        
        let metrics = ErrorCorrectionMetrics {
            original_size: data.len(),
            parity_size: shard_size * self.parity_shards,
            corrections_performed: 0,
            last_correction_time: None,
        };
        
        Ok((shards, metrics))
    }

    pub fn reconstruct(&self, mut shards: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
        // Attempt reconstruction if needed
        self.rs.reconstruct(&mut shards)
            .map_err(|e| format!("Reconstruction failed: {}", e))?;
        
        // Combine data shards
        let mut result = Vec::new();
        for shard in shards.iter().take(self.data_shards) {
            result.extend_from_slice(shard);
        }
        
        Ok(result)
    }
} 