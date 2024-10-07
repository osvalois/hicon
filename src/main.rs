use hybridconsensus::{
    node::Node,
    network::NetworkManager,
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize components
    let network = Arc::new(NetworkManager::new());
    let shard_manager = Arc::new(ShardManager::new());
    let crypto_manager = Arc::new(CryptoManager::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    // Create and run nodes
    let mut handles = vec![];
    for i in 0..5 {
        let node = Node::new(
            format!("node_{}", i),
            Arc::clone(&network),
            Arc::clone(&shard_manager),
            Arc::clone(&crypto_manager),
            Arc::clone(&metrics_collector),
        );
        handles.push(tokio::spawn(async move {
            if let Err(e) = node.run().await {
                tracing::error!("Node {} error: {:?}", i, e);
            }
        }));
    }

    // Wait for all nodes to complete
    for handle in handles {
        handle.await?;
    }

    Ok(())
}