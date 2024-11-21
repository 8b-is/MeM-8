use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a single memory entry in the MeM|8 system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    epoch_pointer: u32,  // 32-bit epoch pointer (136-year span)
    token: u16,         // 16-bit concept encoding
    weight: u16,        // 16-bit importance score
    link1: u32,         // Primary link to related memory
    link2: u32,         // Secondary link to related memory
}

impl MemoryEntry {
    /// Creates a new memory entry with the current epoch
    pub fn new(token: u16, weight: u16) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        Self {
            epoch_pointer: now,
            token,
            weight,
            link1: 0,  // No initial links
            link2: 0,
        }
    }

    /// Creates a memory entry with specific epoch and links
    pub fn with_links(
        epoch_pointer: u32,
        token: u16,
        weight: u16,
        link1: u32,
        link2: u32,
    ) -> Self {
        Self {
            epoch_pointer,
            token,
            weight,
            link1,
            link2,
        }
    }

    // Getters
    pub fn epoch(&self) -> u32 { self.epoch_pointer }
    pub fn token(&self) -> u16 { self.token }
    pub fn weight(&self) -> u16 { self.weight }
    pub fn links(&self) -> (u32, u32) { (self.link1, self.link2) }

    /// Updates the memory links
    pub fn update_links(&mut self, link1: u32, link2: u32) {
        self.link1 = link1;
        self.link2 = link2;
    }

    /// Adjusts the memory weight
    pub fn adjust_weight(&mut self, delta: i16) {
        self.weight = self.weight.saturating_add_signed(delta);
    }

    /// Calculates age in seconds relative to a given epoch
    pub fn age_from(&self, current_epoch: u32) -> u32 {
        current_epoch.saturating_sub(self.epoch_pointer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let entry = MemoryEntry::new(123, 1000);
        assert_eq!(entry.token(), 123);
        assert_eq!(entry.weight(), 1000);
        assert_eq!(entry.links(), (0, 0));
    }

    #[test]
    fn test_weight_adjustment() {
        let mut entry = MemoryEntry::new(123, 1000);
        entry.adjust_weight(500);
        assert_eq!(entry.weight(), 1500);
        entry.adjust_weight(-2000);
        assert_eq!(entry.weight(), 0); // Should saturate at 0
    }

    #[test]
    fn test_link_updates() {
        let mut entry = MemoryEntry::new(123, 1000);
        entry.update_links(42, 84);
        assert_eq!(entry.links(), (42, 84));
    }
} 