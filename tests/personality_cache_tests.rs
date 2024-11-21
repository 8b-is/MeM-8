use mem8::memory::personality_cache::PersonalityCache;
use mem8::memory::entry::MemoryEntry;
use std::collections::HashSet;

#[test]
fn test_cache_add_and_retrieve() {
    let mut cache = PersonalityCache::new(3, 0.5);

    let entry1 = MemoryEntry::new(100, 500);
    let entry2 = MemoryEntry::new(101, 600);

    let related1: HashSet<u16> = [200, 201].into_iter().collect();
    let related2: HashSet<u16> = [202].into_iter().collect();

    cache.add_memory(entry1.clone(), related1.clone());
    cache.add_memory(entry2.clone(), related2.clone());

    // Test retrieval
    assert_eq!(cache.get_memory(entry1.epoch()).unwrap().token(), entry1.token());
    assert_eq!(cache.get_related_tokens(entry1.epoch()).unwrap(), &related1);

    // Test eviction policy
    let entry3 = MemoryEntry::new(102, 700);
    let entry4 = MemoryEntry::new(103, 800);

    cache.add_memory(entry3.clone(), HashSet::new());
    cache.add_memory(entry4.clone(), HashSet::new());

    assert!(cache.get_memory(entry1.epoch()).is_none(), "Entry1 should be evicted");
}