//! Core logic for managing temporal memory entries.

pub struct MemoryEntry {
    pub epoch: u32,       // Epoch pointer (seconds since SeedFile epoch)
    pub token: u16,       // Token ID
    pub weight: i16,      // Importance (-30,000 to 30,000)
    pub link1: Option<u32>, // First link
    pub link2: Option<u32>, // Second link
}

impl MemoryEntry {
    pub fn new(epoch: u32, token: u16, weight: i16, link1: Option<u32>, link2: Option<u32>) -> Self {
        Self {
            epoch,
            token,
            weight,
            link1,
            link2,
        }
    }
}
