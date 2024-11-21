use super::entry::MemoryEntry;
use bincode::{deserialize, serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Stage2Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Memory entry not found: {0}")]
    NotFound(u32),
    #[error("Invalid checksum for entry: {0}")]
    ChecksumMismatch(u32),
}

/// Configuration for Stage2 memory management
#[derive(Debug, Clone)]
pub struct Stage2Config {
    /// Base directory for storing Stage 2 memories
    pub storage_path: PathBuf,
    /// Maximum entries per storage file
    pub entries_per_file: usize,
    /// Minimum age (seconds) before compression
    pub compression_age: u32,
}

impl Default for Stage2Config {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("storage/stage2"),
            entries_per_file: 1000,
            compression_age: 3600 * 24 * 7, // 1 week
        }
    }
}

/// Represents a memory block in Stage 2 storage
#[derive(Serialize, Deserialize)]
struct MemoryBlock {
    entry: MemoryEntry,
    checksum: u32,
    compressed: bool,
}

impl MemoryBlock {
    fn new(entry: MemoryEntry) -> Self {
        let checksum = Self::calculate_checksum(&entry);
        Self {
            entry,
            checksum,
            compressed: false,
        }
    }

    fn calculate_checksum(entry: &MemoryEntry) -> u32 {
        // Simple CRC32 implementation
        let data = serialize(entry).unwrap();
        crc32fast::hash(&data)
    }

    fn verify(&self) -> bool {
        self.checksum == Self::calculate_checksum(&self.entry)
    }
}

pub struct Stage2 {
    config: Stage2Config,
    // In-memory index of epoch -> file location
    index: BTreeMap<u32, (PathBuf, u64)>,
    current_file: Option<File>,
    current_file_entries: usize,
}

impl Stage2 {
    pub fn new(config: Stage2Config) -> io::Result<Self> {
        std::fs::create_dir_all(&config.storage_path)?;
        
        let mut stage2 = Self {
            config,
            index: BTreeMap::new(),
            current_file: None,
            current_file_entries: 0,
        };
        
        stage2.load_index()?;
        Ok(stage2)
    }

    /// Accepts aged entries from Stage 1
    pub fn accept_entries(&mut self, entries: Vec<MemoryEntry>) -> Result<(), Stage2Error> {
        for entry in entries {
            self.store_entry(entry)?;
        }
        Ok(())
    }

    /// Stores a single memory entry
    fn store_entry(&mut self, entry: MemoryEntry) -> Result<(), Stage2Error> {
        // Create new file if needed
        if self.current_file.is_none() || 
           self.current_file_entries >= self.config.entries_per_file {
            self.rotate_file()?;
        }

        let file = self.current_file.as_mut().unwrap();
        let block = MemoryBlock::new(entry);
        
        // Get current position for index
        let pos = file.seek(SeekFrom::End(0))?;
        
        // Write block
        let encoded = serialize(&block)?;
        file.write_all(&encoded)?;
        file.flush()?;

        // Update index
        let current_path = self.current_file_path();
        self.index.insert(block.entry.epoch(), (current_path, pos));
        self.current_file_entries += 1;

        Ok(())
    }

    /// Retrieves a memory entry by epoch
    pub fn get_entry(&mut self, epoch: u32) -> Result<MemoryEntry, Stage2Error> {
        let (path, pos) = self.index.get(&epoch)
            .ok_or(Stage2Error::NotFound(epoch))?;

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(*pos))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let block: MemoryBlock = deserialize(&buffer)?;
        
        if !block.verify() {
            return Err(Stage2Error::ChecksumMismatch(epoch));
        }

        Ok(block.entry)
    }

    /// Compresses old entries to save space
    pub fn compress_old_entries(&mut self) -> Result<(), Stage2Error> {
        let current_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let compression_threshold = current_epoch - self.config.compression_age;
        
        for (&epoch, &(ref path, pos)) in self.index.iter() {
            if epoch < compression_threshold {
                let mut file = File::open(path)?;
                file.seek(SeekFrom::Start(pos))?;
                
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                
                let mut block: MemoryBlock = deserialize(&buffer)?;
                if !block.compressed {
                    // Implement compression logic here
                    block.compressed = true;
                    
                    // Write back compressed block
                    file.seek(SeekFrom::Start(pos))?;
                    let encoded = serialize(&block)?;
                    file.write_all(&encoded)?;
                }
            }
        }
        
        Ok(())
    }

    // Helper methods
    fn rotate_file(&mut self) -> io::Result<()> {
        let path = self.current_file_path();
        self.current_file = Some(OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)?);
        self.current_file_entries = 0;
        Ok(())
    }

    fn current_file_path(&self) -> PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.config.storage_path.join(format!("mem_{}.bin", timestamp))
    }

    fn load_index(&mut self) -> io::Result<()> {
        // Scan directory and rebuild index
        for entry in std::fs::read_dir(&self.config.storage_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "bin") {
                let mut file = File::open(&path)?;
                let mut pos = 0;
                
                loop {
                    let mut buffer = Vec::new();
                    match file.read_to_end(&mut buffer) {
                        Ok(0) => break,
                        Ok(_) => {
                            if let Ok(block) = deserialize::<MemoryBlock>(&buffer) {
                                self.index.insert(block.entry.epoch(), (path.clone(), pos));
                            }
                            pos = file.seek(SeekFrom::Current(0))?;
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_stage2_storage() -> Result<(), Stage2Error> {
        let temp_dir = tempdir().unwrap();
        let config = Stage2Config {
            storage_path: temp_dir.path().to_path_buf(),
            entries_per_file: 10,
            compression_age: 3600,
        };

        let mut stage2 = Stage2::new(config)?;
        
        // Store some entries
        let entries = vec![
            MemoryEntry::new(100, 500),
            MemoryEntry::new(101, 600),
        ];
        
        stage2.accept_entries(entries)?;
        
        // Retrieve and verify
        let entry = stage2.get_entry(100)?;
        assert_eq!(entry.token(), 100);
        assert_eq!(entry.weight(), 500);

        Ok(())
    }
} 