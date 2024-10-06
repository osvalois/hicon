
// benches/throughput.rs
use criterion::{criterion_group, criterion_main, Criterion};
use hybridconsensus::{
    node::Node,
    network::{NetworkManager, transport::InMemoryTransport},
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn benchmark_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("node_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let transport = Arc::new(InMemoryTransport::new());
                let network = Arc::new(NetworkManager::new(Arc::clone(&transport) as Arc<dyn InMemoryTransport>));
                let shard_manager = Arc::new(ShardManager::new());
                let crypto_manager = Arc::new(CryptoManager::new());
                let metrics_collector = Arc::new(MetricsCollector::new());

                let node = Node::new(
                    "bench_node".to_string(),
                    Arc::clone(&network),
                    Arc::clone(&shard_manager),
                    Arc::clone(&crypto_manager),
                    Arc::clone(&metrics_collector),
                );

                let handle = tokio::spawn(async move {
                    if let Err(e) = node.run().await {
                        panic!("Node error: {:?}", e);
                    }
                });

                // Run for a short time to measure throughput
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                handle.abort();
            })
        })
    });
}

criterion_group!(benches, benchmark_throughput);
criterion_main!(benches);