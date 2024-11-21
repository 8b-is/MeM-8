use super::entry::MemoryEntry;
use super::compression::{Compressor, CompressionAlgorithm, CompressionMetrics};
use bincode::{deserialize, serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Stage3Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Core memory not found: {0}")]
    NotFound(u32),
    #[error("Redundancy check failed: {0}")]
    RedundancyError(String),
}

#[derive(Debug, Clone)]
pub struct Stage3Config {
    pub storage_path: PathBuf,
    pub redundancy_path: PathBuf,
    pub compression_algorithm: CompressionAlgorithm,
    pub min_weight_threshold: u16,
    pub min_age_days: u32,
}

impl Default for Stage3Config {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("storage/stage3"),
            redundancy_path: PathBuf::from("storage/stage3_backup"),
            compression_algorithm: CompressionAlgorithm::LZ4,
            min_weight_threshold: 800,  // High importance memories only
            min_age_days: 30,          // At least a month old
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CoreMemoryBlock {
    entry: MemoryEntry,
    metrics: CompressionMetrics,
    checksum: u32,
    parity: Vec<u8>,  // For error correction
}

impl CoreMemoryBlock {
    fn new(entry: MemoryEntry, metrics: CompressionMetrics) -> Self {
        let checksum = Self::calculate_checksum(&entry);
        let parity = Self::generate_parity(&entry);
        Self {
            entry,
            metrics,
            checksum,
            parity,
        }
    }

    fn calculate_checksum(entry: &MemoryEntry) -> u32 {
        let data = serialize(entry).unwrap();
        crc32fast::hash(&data)
    }

    fn generate_parity(entry: &MemoryEntry) -> Vec<u8> {
        let data = serialize(entry).unwrap();
        // Simple XOR-based parity for now
        let mut parity = vec![0u8; 16];  // 128-bit parity
        for (i, &byte) in data.iter().enumerate() {
            parity[i % 16] ^= byte;
        }
        parity
    }

    fn verify(&self) -> bool {
        self.checksum == Self::calculate_checksum(&self.entry)
    }
}

pub struct Stage3 {
    config: Stage3Config,
    index: BTreeMap<u32, (PathBuf, u64)>,
    compressor: Compressor,
}

impl Stage3 {
    pub fn new(config: Stage3Config) -> io::Result<Self> {
        std::fs::create_dir_all(&config.storage_path)?;
        std::fs::create_dir_all(&config.redundancy_path)?;
        
        Ok(Self {
            compressor: Compressor::new(config.compression_algorithm),
            index: BTreeMap::new(),
            config,
        })
    }

    /// Evaluates Stage 2 entries for promotion to Stage 3
    pub fn evaluate_promotion(&self, entry: &MemoryEntry, age_days: u32) -> bool {
        age_days >= self.config.min_age_days && 
        entry.weight() >= self.config.min_weight_threshold
    }

    /// Stores a core memory with redundancy
    pub fn store_core_memory(&mut self, entry: MemoryEntry) -> Result<(), Stage3Error> {
        let data = serialize(&entry)?;
        let (compressed_data, metrics) = self.compressor.compress(&data);
        
        let block = CoreMemoryBlock::new(entry, metrics);
        let encoded = serialize(&block)?;

        // Store primary copy
        let primary_path = self.get_storage_path(block.entry.epoch());
        let mut primary_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(primary_path.clone())?;

        primary_file.write_all(&encoded)?;

        // Store backup copy
        let backup_path = self.get_backup_path(block.entry.epoch());
        let mut backup_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(backup_path)?;

        backup_file.write_all(&encoded)?;

        // Update index
        self.index.insert(block.entry.epoch(), (primary_path, 0));

        Ok(())
    }

    /// Retrieves a core memory with redundancy check
    pub fn get_core_memory(&self, epoch: u32) -> Result<MemoryEntry, Stage3Error> {
        let (primary_path, _) = self.index.get(&epoch)
            .ok_or(Stage3Error::NotFound(epoch))?;

        let backup_path = self.get_backup_path(epoch);

        // Try primary first
        match self.read_memory_block(primary_path) {
            Ok(block) if block.verify() => Ok(block.entry),
            _ => {
                // Try backup if primary fails
                match self.read_memory_block(&backup_path) {
                    Ok(block) if block.verify() => {
                        // Repair primary from backup
                        self.repair_primary(epoch, &block)?;
                        Ok(block.entry)
                    }
                    _ => Err(Stage3Error::RedundancyError(
                        format!("Both primary and backup copies corrupted for epoch {}", epoch)
                    )),
                }
            }
        }
    }

    // Helper methods
    fn get_storage_path(&self, epoch: u32) -> PathBuf {
        self.config.storage_path.join(format!("core_{}.bin", epoch))
    }

    fn get_backup_path(&self, epoch: u32) -> PathBuf {
        self.config.redundancy_path.join(format!("core_{}.bin", epoch))
    }

    fn read_memory_block(&self, path: &PathBuf) -> Result<CoreMemoryBlock, Stage3Error> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(deserialize(&buffer)?)
    }

    fn repair_primary(&self, epoch: u32, block: &CoreMemoryBlock) -> Result<(), Stage3Error> {
        let primary_path = self.get_storage_path(epoch);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(primary_path)?;
        
        let encoded = serialize(block)?;
        file.write_all(&encoded)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_core_memory_storage() -> Result<(), Stage3Error> {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();

        let config = Stage3Config {
            storage_path: temp_dir.path().to_path_buf(),
            redundancy_path: backup_dir.path().to_path_buf(),
            ..Stage3Config::default()
        };

        let mut stage3 = Stage3::new(config)?;
        
        // Create a high-weight memory
        let entry = MemoryEntry::new(100, 900);
        
        // Store it
        stage3.store_core_memory(entry.clone())?;
        
        // Retrieve and verify
        let retrieved = stage3.get_core_memory(entry.epoch())?;
        assert_eq!(retrieved.token(), entry.token());
        assert_eq!(retrieved.weight(), entry.weight());

        Ok(())
    }

    #[test]
    fn test_redundancy_recovery() -> Result<(), Stage3Error> {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();

        let config = Stage3Config {
            storage_path: temp_dir.path().to_path_buf(),
            redundancy_path: backup_dir.path().to_path_buf(),
            ..Stage3Config::default()
        };

        let mut stage3 = Stage3::new(config)?;
        
        // Store a memory
        let entry = MemoryEntry::new(100, 900);
        stage3.store_core_memory(entry.clone())?;
        
        // Corrupt primary file
        let primary_path = stage3.get_storage_path(entry.epoch());
        let mut file = OpenOptions::new()
            .write(true)
            .open(primary_path)?;
        file.write_all(&[0; 100])?;
        
        // Should still retrieve from backup
        let retrieved = stage3.get_core_memory(entry.epoch())?;
        assert_eq!(retrieved.token(), entry.token());

        Ok(())
    }
} 