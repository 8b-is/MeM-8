use criterion::{criterion_group, criterion_main, Criterion};
use mem8::memory::{MemoryCache, MemoryEntry};
use std::collections::HashSet;
use criterion::BenchmarkId;
use criterion::Throughput;

fn benchmark_cache_retrieval(c: &mut Criterion) {
    let mut cache = MemoryCache::new(100);

    // Add memories to cache
    for i in 0..100 {
        let entry = MemoryEntry::new(i as u16, 500);
        let related: HashSet<u16> = [(i + 1) as u16].into_iter().collect();
        cache.add_memory(entry, related);
    }

    c.bench_function("cache retrieval", |b| {
        b.iter(|| {
            cache.get_memory(50);
        });
    });
}

// Add more benchmark functions here
fn benchmark_cache_insertion(c: &mut Criterion) {
    let mut cache = MemoryCache::new(100);
    let mut i = 0;

    c.bench_function("cache insertion", |b| {
        b.iter(|| {
            let entry = MemoryEntry::new(i as u16, 500);
            let related: HashSet<u16> = [(i + 1) as u16].into_iter().collect();
            cache.add_memory(entry, related);
            i += 1;
        });
    });
}

fn benchmark_cache_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_sizes");
    
    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut cache = MemoryCache::new(size);
            // Setup cache with 'size' elements
            for i in 0..size {
                let entry = MemoryEntry::new(i as u16, 500);
                let related: HashSet<u16> = [(i + 1) as u16].into_iter().collect();
                cache.add_memory(entry, related);
            }
            
            b.iter(|| {
                for i in 0..size {
                    cache.get_memory(i as u32);
                }
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_cache_retrieval,
    benchmark_cache_insertion,
    benchmark_cache_sizes
);
criterion_main!(benches); 