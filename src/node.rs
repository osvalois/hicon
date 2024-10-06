// src/node.rs
use crate::{
    ccr::CCRLayer,
    ccc::CCCLayer,
    network::NetworkManager,
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
    Result,
};
use tokio::sync::mpsc;
use std::sync::Arc;
use dashmap::DashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

pub struct Node {
    id: String,
    state: Arc<tokio::sync::RwLock<NodeState>>,
    ccr_layer: Arc<CCRLayer>,
    ccc_layer: Arc<CCCLayer>,
    network: Arc<NetworkManager>,
    shard_manager: Arc<ShardManager>,
    crypto_manager: Arc<CryptoManager>,
    metrics_collector: Arc<MetricsCollector>,
    peers: Arc<DashMap<String, mpsc::Sender<Vec<u8>>>>,
}

impl Node {
    pub fn new(
        id: String,
        network: Arc<NetworkManager>,
        shard_manager: Arc<ShardManager>,
        crypto_manager: Arc<CryptoManager>,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Self {
        Node {
            id,
            state: Arc::new(tokio::sync::RwLock::new(NodeState::Follower)),
            ccr_layer: Arc::new(CCRLayer::new()),
            ccc_layer: Arc::new(CCCLayer::new()),
            network,
            shard_manager,
            crypto_manager,
            metrics_collector,
            peers: Arc::new(DashMap::new()),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(1000);
        self.network.register_node(self.id.clone(), tx).await?;

        loop {
            tokio::select! {
                Some(message) = rx.recv() => {
                    self.handle_message(message).await?;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    self.check_timeouts().await?;
                }
            }
        }
    }

    async fn handle_message(&self, message: Vec<u8>) -> Result<()> {
        let message = self.crypto_manager.decrypt(&message)?;
        // Implement message handling logic
        Ok(())
    }

    async fn check_timeouts(&self) -> Result<()> {
        let state = *self.state.read().await;
        match state {
            NodeState::Follower => {
                if self.ccr_layer.should_become_candidate().await {
                    self.start_election().await?;
                }
            }
            NodeState::Candidate => {
                if self.ccr_layer.election_timeout_elapsed().await {
                    self.start_election().await?;
                }
            }
            NodeState::Leader => {
                self.send_heartbeat().await?;
            }
        }
        Ok(())
    }

    async fn start_election(&self) -> Result<()> {
        *self.state.write().await = NodeState::Candidate;
        // Implement election logic
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        // Implement heartbeat sending logic
        Ok(())
    }
}
