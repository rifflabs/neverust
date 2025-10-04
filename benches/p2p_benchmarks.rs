use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use neverust_core::{BlockStore, Block, create_swarm, Metrics};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark: Block creation and CID generation
fn bench_block_creation(c: &mut Criterion) {
    c.bench_function("block_creation_1kb", |b| {
        let data = vec![0u8; 1024]; // 1 KB
        b.iter(|| {
            black_box(Block::new(data.clone()).unwrap())
        });
    });

    c.bench_function("block_creation_1mb", |b| {
        let data = vec![0u8; 1024 * 1024]; // 1 MB
        b.iter(|| {
            black_box(Block::new(data.clone()).unwrap())
        });
    });

    c.bench_function("block_creation_10mb", |b| {
        let data = vec![0u8; 10 * 1024 * 1024]; // 10 MB
        b.iter(|| {
            black_box(Block::new(data.clone()).unwrap())
        });
    });
}

/// Benchmark: BlockStore operations (in-memory)
fn bench_block_store(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("blockstore_put_1kb", |b| {
        let store = Arc::new(BlockStore::new());
        let block = Block::new(vec![0u8; 1024]).unwrap();

        b.to_async(&rt).iter(|| async {
            black_box(store.put(block.clone()).await.unwrap())
        });
    });

    c.bench_function("blockstore_get_1kb", |b| {
        let store = Arc::new(BlockStore::new());
        let block = Block::new(vec![0u8; 1024]).unwrap();
        let cid = block.cid;
        rt.block_on(async { store.put(block).await.unwrap() });

        b.to_async(&rt).iter(|| async {
            black_box(store.get(&cid).await.unwrap())
        });
    });

    c.bench_function("blockstore_has_1kb", |b| {
        let store = Arc::new(BlockStore::new());
        let block = Block::new(vec![0u8; 1024]).unwrap();
        let cid = block.cid;
        rt.block_on(async { store.put(block).await.unwrap() });

        b.to_async(&rt).iter(|| async {
            black_box(store.has(&cid).await)
        });
    });
}

/// Benchmark: P2P swarm creation
fn bench_swarm_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("swarm_creation", |b| {
        b.to_async(&rt).iter(|| async {
            let block_store = Arc::new(BlockStore::new());
            let metrics = Metrics::new();
            black_box(
                create_swarm(
                    block_store,
                    "altruistic".to_string(),
                    0,
                    metrics
                ).await.unwrap()
            )
        });
    });
}

/// Benchmark: Metrics operations
fn bench_metrics(c: &mut Criterion) {
    c.bench_function("metrics_record_block_sent", |b| {
        let metrics = Metrics::new();
        b.iter(|| {
            black_box(metrics.block_sent(1024))
        });
    });

    c.bench_function("metrics_record_peer_connected", |b| {
        let metrics = Metrics::new();
        b.iter(|| {
            black_box(metrics.peer_connected())
        });
    });

    c.bench_function("metrics_to_prometheus", |b| {
        let metrics = Metrics::new();
        // Add some data
        for _ in 0..100 {
            metrics.peer_connected();
            metrics.block_sent(1024);
            metrics.block_received(2048);
        }

        b.iter(|| {
            black_box(metrics.to_prometheus(100, 1024000))
        });
    });
}

/// Benchmark: Concurrent block operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_operations");

    for &num_tasks in &[1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_puts", num_tasks),
            &num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async move {
                    let store = Arc::new(BlockStore::new());
                    let mut handles = Vec::new();

                    for i in 0..num_tasks {
                        let store = store.clone();
                        let handle = tokio::spawn(async move {
                            let data = vec![i as u8; 1024];
                            let block = Block::new(data).unwrap();
                            store.put(block).await.unwrap()
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_block_creation,
    bench_block_store,
    bench_swarm_creation,
    bench_metrics,
    bench_concurrent_operations
);
criterion_main!(benches);
