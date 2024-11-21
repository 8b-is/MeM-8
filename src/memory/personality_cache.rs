use super::entry::MemoryEntry;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::time::{SystemTime, Duration};
use parking_lot::RwLock;

/// Represents the importance of a memory in the personality matrix
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PersonalityScore {
    weight: u16,
    access_count: u32,
    link_strength: f32,
    last_access: SystemTime,
}

pub struct PersonalityCache {
    entries: RwLock<HashMap<u32, (MemoryEntry, PersonalityScore)>>,
    token_index: RwLock<BTreeMap<u16, HashSet<u32>>>,  // Token -> Epochs mapping
    max_entries: usize,
    personality_threshold: f32,
}

impl PersonalityCache {
    pub fn new(max_entries: usize, personality_threshold: f32) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            token_index: RwLock::new(BTreeMap::new()),
            max_entries,
            personality_threshold,
        }
    }

    /// Adds or updates a memory in the personality cache
    pub fn update_memory(&self, entry: MemoryEntry, related_tokens: HashSet<u16>) -> bool {
        let mut entries = self.entries.write();
        let mut token_index = self.token_index.write();

        let epoch = entry.epoch();
        let score = self.calculate_personality_score(&entry, &related_tokens);

        // Only cache if the personality score meets our threshold
        if score.link_strength >= self.personality_threshold {
            if entries.len() >= self.max_entries {
                self.evict_lowest_scoring(&mut entries, &mut token_index);
            }

            // Update token index
            token_index
                .entry(entry.token())
                .or_default()
                .insert(epoch);

            for token in related_tokens {
                token_index
                    .entry(token)
                    .or_default()
                    .insert(epoch);
            }

            entries.insert(epoch, (entry, score));
            true
        } else {
            false
        }
    }

    /// Retrieves a memory and updates its access metrics
    pub fn get_memory(&self, epoch: u32) -> Option<MemoryEntry> {
        let mut entries = self.entries.write();
        
        if let Some((entry, score)) = entries.get_mut(&epoch) {
            let mut updated_score = *score;
            updated_score.access_count += 1;
            updated_score.last_access = SystemTime::now();
            
            entries.insert(epoch, (entry.clone(), updated_score));
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Finds related memories based on token patterns
    pub fn find_related_memories(&self, token: u16, limit: usize) -> Vec<MemoryEntry> {
        let token_index = self.token_index.read();
        let entries = self.entries.read();
        
        if let Some(epochs) = token_index.get(&token) {
            epochs.iter()
                .filter_map(|&epoch| entries.get(&epoch))
                .map(|(entry, _)| entry.clone())
                .take(limit)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the personality relevance score for a memory
    fn calculate_personality_score(
        &self, 
        entry: &MemoryEntry, 
        related_tokens: &HashSet<u16>
    ) -> PersonalityScore {
        let entries = self.entries.read();
        let (link1, link2) = entry.links();
        
        // Calculate link strength based on connected memories
        let link_strength = [link1, link2].iter()
            .filter(|&&link| link != 0)
            .filter_map(|&link| entries.get(&link))
            .map(|(_, score)| score.weight as f32 / u16::MAX as f32)
            .sum::<f32>() / 2.0;

        PersonalityScore {
            weight: entry.weight(),
            access_count: 0,
            link_strength,
            last_access: SystemTime::now(),
        }
    }

    /// Evicts the lowest scoring entry from the cache
    fn evict_lowest_scoring(
        &self,
        entries: &mut HashMap<u32, (MemoryEntry, PersonalityScore)>,
        token_index: &mut BTreeMap<u16, HashSet<u32>>
    ) {
        if let Some((&epoch, _)) = entries.iter()
            .min_by(|&(_, (_, a)), &(_, (_, b))| {
                let a_score = a.weight as f32 * a.link_strength;
                let b_score = b.weight as f32 * b.link_strength;
                a_score.partial_cmp(&b_score).unwrap()
            }) 
        {
            if let Some((entry, _)) = entries.remove(&epoch) {
                // Clean up token index
                if let Some(epochs) = token_index.get_mut(&entry.token()) {
                    epochs.remove(&epoch);
                }
            }
        }
    }

    /// Returns cache statistics
    pub fn stats(&self) -> CacheStats {
        let entries = self.entries.read();
        
        CacheStats {
            total_entries: entries.len(),
            avg_weight: entries.values()
                .map(|(_, score)| score.weight as f32)
                .sum::<f32>() / entries.len() as f32,
            avg_link_strength: entries.values()
                .map(|(_, score)| score.link_strength)
                .sum::<f32>() / entries.len() as f32,
            cache_hit_rate: 0.0, // TODO: Implement hit rate tracking
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub avg_weight: f32,
    pub avg_link_strength: f32,
    pub cache_hit_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_cache_add_and_retrieve() {
        let cache = PersonalityCache::new(3, 0.5);

        let entry1 = MemoryEntry::new(100, 500);
        let entry2 = MemoryEntry::new(101, 600);

        let related1: HashSet<u16> = [200, 201].into_iter().collect();
        let related2: HashSet<u16> = [202].into_iter().collect();

        cache.update_memory(entry1.clone(), related1.clone());
        cache.update_memory(entry2.clone(), related2.clone());

        // Test retrieval
        let retrieved1 = cache.get_memory(entry1.epoch()).unwrap();
        assert_eq!(retrieved1.token(), entry1.token());
        
        // Test related tokens
        let related_memories = cache.find_related_memories(200, 10);
        assert!(related_memories.iter().any(|e| e.epoch() == entry1.epoch()));

        // Test eviction policy
        let entry3 = MemoryEntry::new(102, 700);
        let entry4 = MemoryEntry::new(103, 800);

        cache.update_memory(entry3.clone(), HashSet::new());
        cache.update_memory(entry4.clone(), HashSet::new());

        assert!(cache.get_memory(entry1.epoch()).is_none(), "Entry1 should be evicted");
    }

    // Add more comprehensive tests for personality aspects
    #[test]
    fn test_personality_weighted_eviction() {
        let cache = PersonalityCache::new(3, 0.5);

        // Create entries with different weights
        let mut entry1 = MemoryEntry::new(100, 900); // High weight
        let mut entry2 = MemoryEntry::new(101, 300); // Low weight
        let mut entry3 = MemoryEntry::new(102, 600); // Medium weight

        // Create links between entries
        entry1.update_links(entry2.epoch(), entry3.epoch());
        entry2.update_links(entry1.epoch(), 0);
        entry3.update_links(entry1.epoch(), 0);

        let related: HashSet<u16> = vec![100, 101, 102].into_iter().collect();

        // Add all entries
        cache.update_memory(entry1.clone(), related.clone());
        cache.update_memory(entry2.clone(), related.clone());
        cache.update_memory(entry3.clone(), related.clone());

        // Add a new entry to trigger eviction
        let entry4 = MemoryEntry::new(104, 950);
        cache.update_memory(entry4.clone(), HashSet::new());

        // The lowest weight entry should be evicted
        assert!(cache.get_memory(entry2.epoch()).is_none(), "Low weight entry should be evicted");
        assert!(cache.get_memory(entry1.epoch()).is_some(), "High weight entry should remain");
    }

    #[test]
    fn test_access_patterns() {
        let cache = PersonalityCache::new(3, 0.5);
        let entry = MemoryEntry::new(100, 500);
        let related: HashSet<u16> = [200, 201].into_iter().collect();

        cache.update_memory(entry.clone(), related);

        // Access the entry multiple times
        for _ in 0..5 {
            cache.get_memory(entry.epoch());
            sleep(Duration::from_millis(10));
        }

        let stats = cache.stats();
        assert!(stats.avg_weight > 0.0, "Average weight should be positive");
        assert!(stats.total_entries > 0, "Cache should contain entries");
    }

    // Your existing personality scoring test remains...
    #[test]
    fn test_personality_scoring() {
        let cache = PersonalityCache::new(10, 0.5);
        
        // Create a network of related memories
        let mut entry1 = MemoryEntry::new(100, 900);
        let mut entry2 = MemoryEntry::new(101, 800);
        let entry3 = MemoryEntry::new(102, 700);
        
        // Link memories
        entry1.update_links(entry2.epoch(), entry3.epoch());
        entry2.update_links(entry1.epoch(), entry3.epoch());
        
        let related: HashSet<u16> = vec![100, 101, 102].into_iter().collect();
        
        // Add to cache
        assert!(cache.update_memory(entry1.clone(), related.clone()));
        assert!(cache.update_memory(entry2.clone(), related.clone()));
        assert!(cache.update_memory(entry3.clone(), related.clone()));
        
        // Verify retrieval and scoring
        let retrieved = cache.get_memory(entry1.epoch()).unwrap();
        assert_eq!(retrieved.token(), entry1.token());
        
        let stats = cache.stats();
        assert!(stats.avg_link_strength > 0.0);
    }

    // Your existing cache eviction test remains...
    #[test]
    fn test_cache_eviction() {
        let cache = PersonalityCache::new(2, 0.5);
        
        // Add three entries to trigger eviction
        let entry1 = MemoryEntry::new(100, 900);
        let entry2 = MemoryEntry::new(101, 800);
        let entry3 = MemoryEntry::new(102, 950);
        
        let related: HashSet<u16> = vec![100, 101, 102].into_iter().collect();
        
        cache.update_memory(entry1.clone(), related.clone());
        cache.update_memory(entry2.clone(), related.clone());
        cache.update_memory(entry3.clone(), related.clone());
        
        // Verify lowest scoring entry was evicted
        assert!(cache.get_memory(entry2.epoch()).is_none());
        assert!(cache.get_memory(entry1.epoch()).is_some());
        assert!(cache.get_memory(entry3.epoch()).is_some());
    }
} 