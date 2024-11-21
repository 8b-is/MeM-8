use super::entry::MemoryEntry;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Stage1Error {
    #[error("Memory entry not found for epoch {0}")]
    EntryNotFound(u32),
    #[error("Invalid link: target epoch {0} does not exist")]
    InvalidLink(u32),
}

/// Configuration for Stage1 memory management
#[derive(Debug, Clone)]
pub struct Stage1Config {
    /// Maximum age (in seconds) before memory is eligible for cleanup
    pub max_age: u32,
    /// Minimum weight threshold for retention
    pub min_weight: u16,
    /// Weight decay rate (per hour)
    pub decay_rate: f32,
    /// Token similarity threshold for automatic linking
    pub similarity_threshold: f32,
}

impl Default for Stage1Config {
    fn default() -> Self {
        Self {
            max_age: 3600 * 24,  // 24 hours
            min_weight: 100,
            decay_rate: 0.95,    // 5% decay per hour
            similarity_threshold: 0.7,
        }
    }
}

/// High-resolution, ephemeral memory storage
pub struct Stage1 {
    entries: HashMap<u32, MemoryEntry>,
    current_epoch: u32,
    config: Stage1Config,
    last_cleanup: u32,
}

impl Stage1 {
    /// Creates a new Stage1 memory instance
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            current_epoch: 0,
            config: Stage1Config::default(),
            last_cleanup: 0,
        }
    }

    /// Adds a new memory entry
    pub fn add_memory(&mut self, token: u16, weight: u16) -> u32 {
        let entry = MemoryEntry::new(token, weight);
        let epoch = entry.epoch();
        self.entries.insert(epoch, entry);
        self.current_epoch = epoch;
        epoch
    }

    /// Retrieves a memory by its epoch
    pub fn get_memory(&self, epoch: u32) -> Result<&MemoryEntry, Stage1Error> {
        self.entries
            .get(&epoch)
            .ok_or(Stage1Error::EntryNotFound(epoch))
    }

    /// Links two memories together
    pub fn link_memories(
        &mut self,
        source_epoch: u32,
        link1: u32,
        link2: u32,
    ) -> Result<(), Stage1Error> {
        // Verify links exist
        if link1 != 0 && !self.entries.contains_key(&link1) {
            return Err(Stage1Error::InvalidLink(link1));
        }
        if link2 != 0 && !self.entries.contains_key(&link2) {
            return Err(Stage1Error::InvalidLink(link2));
        }

        // Update links
        if let Some(entry) = self.entries.get_mut(&source_epoch) {
            entry.update_links(link1, link2);
            Ok(())
        } else {
            Err(Stage1Error::EntryNotFound(source_epoch))
        }
    }

    /// Returns all memories older than the specified age in seconds
    pub fn get_aged_memories(&self, min_age_seconds: u32) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|entry| entry.age_from(self.current_epoch) >= min_age_seconds)
            .collect()
    }

    /// Performs memory cleanup and weight decay
    pub fn maintain(&mut self) -> Vec<MemoryEntry> {
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let hours_since_cleanup = (current_epoch - self.last_cleanup) / 3600;
        let decay_factor = self.config.decay_rate.powi(hours_since_cleanup as i32);

        // Collect entries for removal or transition to Stage 2
        let mut to_remove = Vec::new();
        let mut aged_entries = Vec::new();

        for (epoch, entry) in self.entries.iter_mut() {
            // Apply weight decay
            let new_weight = (entry.weight() as f32 * decay_factor) as u16;
            entry.adjust_weight((new_weight as i16) - (entry.weight() as i16));

            // Check for removal conditions
            if entry.age_from(current_epoch) > self.config.max_age 
               || entry.weight() < self.config.min_weight {
                to_remove.push(*epoch);
                aged_entries.push(entry.clone());
            }
        }

        // Remove processed entries
        for epoch in to_remove {
            self.entries.remove(&epoch);
        }

        self.last_cleanup = current_epoch;
        aged_entries
    }

    /// Attempts to find and create links between similar memories
    pub fn update_automatic_links(&mut self) {
        let epochs: Vec<u32> = self.entries.keys().cloned().collect();
        
        for &source_epoch in &epochs {
            let mut best_matches = Vec::new();
            let source_token = self.entries[&source_epoch].token();

            // Find similar memories
            for &target_epoch in &epochs {
                if source_epoch != target_epoch {
                    let target_token = self.entries[&target_epoch].token();
                    let similarity = Self::calculate_similarity(source_token, target_token);
                    
                    if similarity >= self.config.similarity_threshold {
                        best_matches.push((target_epoch, similarity));
                    }
                }
            }

            // Sort by similarity and update links
            best_matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            if let Some(entry) = self.entries.get_mut(&source_epoch) {
                let link1 = best_matches.get(0).map(|&(epoch, _)| epoch).unwrap_or(0);
                let link2 = best_matches.get(1).map(|&(epoch, _)| epoch).unwrap_or(0);
                entry.update_links(link1, link2);
            }
        }
    }

    /// Calculate similarity between two tokens (simple example)
    fn calculate_similarity(token1: u16, token2: u16) -> f32 {
        // This is a simple example - replace with your similarity metric
        let diff = (token1 as i32 - token2 as i32).abs();
        let max_diff = u16::MAX as i32;
        1.0 - (diff as f32 / max_diff as f32)
    }

    /// Returns statistics about the current memory state
    pub fn stats(&self) -> Stage1Stats {
        let current_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        Stage1Stats {
            total_entries: self.entries.len(),
            avg_weight: self.entries.values()
                .map(|e| e.weight() as f32)
                .sum::<f32>() / self.entries.len() as f32,
            avg_age: self.entries.values()
                .map(|e| e.age_from(current_epoch) as f32)
                .sum::<f32>() / self.entries.len() as f32,
            linked_entries: self.entries.values()
                .filter(|e| e.links() != (0, 0))
                .count(),
        }
    }
}

#[derive(Debug)]
pub struct Stage1Stats {
    pub total_entries: usize,
    pub avg_weight: f32,
    pub avg_age: f32,
    pub linked_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_memory_storage_and_retrieval() {
        let mut stage1 = Stage1::new();
        let epoch = stage1.add_memory(123, 1000);
        
        let entry = stage1.get_memory(epoch).unwrap();
        assert_eq!(entry.token(), 123);
        assert_eq!(entry.weight(), 1000);
    }

    #[test]
    fn test_memory_linking() {
        let mut stage1 = Stage1::new();
        let epoch1 = stage1.add_memory(123, 1000);
        let epoch2 = stage1.add_memory(456, 2000);
        
        stage1.link_memories(epoch2, epoch1, 0).unwrap();
        
        let entry = stage1.get_memory(epoch2).unwrap();
        assert_eq!(entry.links(), (epoch1, 0));
    }

    #[test]
    fn test_memory_decay() {
        let mut stage1 = Stage1::new();
        let epoch = stage1.add_memory(123, 1000);
        
        // Force decay
        sleep(Duration::from_secs(1));
        let aged = stage1.maintain();
        
        let entry = stage1.get_memory(epoch).unwrap();
        assert!(entry.weight() < 1000, "Weight should decay over time");
    }

    #[test]
    fn test_automatic_linking() {
        let mut stage1 = Stage1::new();
        let epoch1 = stage1.add_memory(100, 1000);
        let epoch2 = stage1.add_memory(101, 1000);  // Similar token
        let epoch3 = stage1.add_memory(500, 1000);  // Different token
        
        stage1.update_automatic_links();
        
        let entry1 = stage1.get_memory(epoch1).unwrap();
        let (link1, _) = entry1.links();
        assert_eq!(link1, epoch2, "Should link to similar token");
    }
} 