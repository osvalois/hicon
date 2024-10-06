use hybridconsensus::{
    node::Node,
    network::{NetworkManager, transport::InMemoryTransport},
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
    let transport = Arc::new(InMemoryTransport::new());
    let network = Arc::new(NetworkManager::new(Arc::clone(&transport) as Arc<dyn InMemoryTransport>));
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
        // Continuación de src/main.rs
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

// tests/integration_tests.rs
use hybridconsensus::{
    node::Node,
    network::{NetworkManager, transport::InMemoryTransport},
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
};
use std::sync::Arc;

#[tokio::test]
async fn test_node_startup() {
    let transport = Arc::new(InMemoryTransport::new());
    let network = Arc::new(NetworkManager::new(Arc::clone(&transport) as Arc<dyn InMemoryTransport>));
    let shard_manager = Arc::new(ShardManager::new());
    let crypto_manager = Arc::new(CryptoManager::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let node = Node::new(
        "test_node".to_string(),
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

    // Run for a short time to ensure startup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Cancel the node's run task
    handle.abort();

    // Assert that the node started successfully
    assert!(handle.await.unwrap_err().is_cancelled());
}

// tests/stress_tests.rs
use hybridconsensus::{
    node::Node,
    network::{NetworkManager, transport::InMemoryTransport},
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
};
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::test]
async fn test_high_load() {
    let transport = Arc::new(InMemoryTransport::new());
    let network = Arc::new(NetworkManager::new(Arc::clone(&transport) as Arc<dyn InMemoryTransport>));
    let shard_manager = Arc::new(ShardManager::new());
    let crypto_manager = Arc::new(CryptoManager::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let mut handles = vec![];
    for i in 0..10 {
        let node = Node::new(
            format!("node_{}", i),
            Arc::clone(&network),
            Arc::clone(&shard_manager),
            Arc::clone(&crypto_manager),
            Arc::clone(&metrics_collector),
        );
        handles.push(tokio::spawn(async move {
            if let Err(e) = node.run().await {
                panic!("Node {} error: {:?}", i, e);
            }
        }));
    }

    // Simulate high load for 10 seconds
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Cancel all node tasks
    for handle in handles {
        handle.abort();
    }

    // Assert that all nodes ran without panicking
    for handle in handles {
        assert!(handle.await.unwrap_err().is_cancelled());
    }
}
