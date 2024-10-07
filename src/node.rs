// src/node.rs
use crate::{
    ccr::CCRLayer,
    ccc::CCCLayer,
    network::NetworkManager,
    sharding::ShardManager,
    crypto::CryptoManager,
    metrics::MetricsCollector,
    consensus::{
        ConsensusMessage, 
        ConsensusState, 
        RequestVote, 
        VoteResponse, 
        AppendEntries, 
        AppendEntriesResponse
    },
    HybridConsensusError, Result,
};
use tokio::sync::mpsc;
use std::sync::Arc;
use dashmap::DashMap;
use serde_json;
use tracing::{info, error, debug};

pub struct Node {
    id: String,
    state: Arc<tokio::sync::RwLock<ConsensusState>>,
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
        let (ccr_sender, ccr_receiver) = mpsc::channel(1000);
        let (ccc_sender, ccc_receiver) = mpsc::channel(1000);

        let ccc_layer = CCCLayer::new(id.clone(), 3, ccc_sender, ccc_receiver);

        Node {
            id: id.clone(),
            state: Arc::new(tokio::sync::RwLock::new(ConsensusState::Follower)),
            ccr_layer: Arc::new(CCRLayer::new()),
            ccc_layer: Arc::new(ccc_layer),
            network,
            shard_manager,
            crypto_manager,
            metrics_collector,
            peers: Arc::new(DashMap::new()),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(1000);
        self.network.register_node(self.id.clone(), tx).await.map_err(|e| HybridConsensusError::NetworkError(e.to_string()))?;

        info!("Node {} started", self.id);

        loop {
            tokio::select! {
                Some(message) = rx.recv() => {
                    if let Err(e) = self.handle_message(message).await {
                        error!("Error handling message: {:?}", e);
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    if let Err(e) = self.check_timeouts().await {
                        error!("Error checking timeouts: {:?}", e);
                    }
                }
            }
        }
    }

    async fn handle_message(&self, encrypted_message: Vec<u8>) -> Result<()> {
        let message = self.crypto_manager.decrypt(&encrypted_message).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
        let consensus_message: ConsensusMessage = serde_json::from_slice(&message)
            .map_err(|e| HybridConsensusError::SerializationError(e.to_string()))?;

        debug!("Handling message: {:?}", consensus_message);

        match consensus_message {
            ConsensusMessage::RequestVote(request) => self.handle_request_vote(request).await?,
            ConsensusMessage::VoteResponse(response) => self.handle_vote_response(response).await?,
            ConsensusMessage::AppendEntries(request) => self.handle_append_entries(request).await?,
            ConsensusMessage::AppendEntriesResponse(response) => self.handle_append_entries_response(response).await?,
        }

        self.metrics_collector.record_message_received();
        Ok(())
    }

    async fn check_timeouts(&self) -> Result<()> {
        let state = self.state.read().await.clone();
        match state {
            ConsensusState::Follower => {
                if self.ccr_layer.should_become_candidate().await {
                    self.start_election().await?;
                }
            }
            ConsensusState::Candidate => {
                if self.ccr_layer.election_timeout_elapsed().await {
                    self.start_election().await?;
                }
            }
            ConsensusState::Leader => {
                self.send_heartbeat().await?;
            }
        }
        Ok(())
    }

    async fn start_election(&self) -> Result<()> {
        info!("Node {} starting election", self.id);
        *self.state.write().await = ConsensusState::Candidate;
        self.ccr_layer.increment_term().await;
        self.request_votes().await?;
        Ok(())
    }

    async fn request_votes(&self) -> Result<()> {
        let request = self.ccr_layer.create_vote_request().await;
        for peer in self.peers.iter() {
            let encrypted_request = self.crypto_manager.encrypt(&serde_json::to_vec(&request).map_err(|e| HybridConsensusError::SerializationError(e.to_string()))?).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
            self.network.send_message(peer.key(), encrypted_request).await.map_err(|e| HybridConsensusError::NetworkError(e.to_string()))?;
        }
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        let heartbeat = self.ccr_layer.create_heartbeat().await;
        for peer in self.peers.iter() {
            let encrypted_heartbeat = self.crypto_manager.encrypt(&serde_json::to_vec(&heartbeat).map_err(|e| HybridConsensusError::SerializationError(e.to_string()))?).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
            self.network.send_message(peer.key(), encrypted_heartbeat).await.map_err(|e| HybridConsensusError::NetworkError(e.to_string()))?;
        }
        Ok(())
    }

    async fn handle_request_vote(&self, request: RequestVote) -> Result<()> {
        let response = self.ccr_layer.handle_vote_request(request).await;
        let encrypted_response = self.crypto_manager.encrypt(&serde_json::to_vec(&response).map_err(|e| HybridConsensusError::SerializationError(e.to_string()))?).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
        self.network.send_message(&response.candidate_id, encrypted_response).await.map_err(|e| HybridConsensusError::NetworkError(e.to_string()))?;
        Ok(())
    }

    async fn handle_vote_response(&self, response: VoteResponse) -> Result<()> {
        self.ccr_layer.handle_vote_response(response).await;
        if self.ccr_layer.has_majority_votes().await {
            info!("Node {} became leader", self.id);
            *self.state.write().await = ConsensusState::Leader;
            self.send_heartbeat().await?;
        }
        Ok(())
    }

    async fn handle_append_entries(&self, request: AppendEntries) -> Result<()> {
        let response = self.ccr_layer.handle_append_entries(request).await;
        let encrypted_response = self.crypto_manager.encrypt(&serde_json::to_vec(&response).map_err(|e| HybridConsensusError::SerializationError(e.to_string()))?).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
        self.network.send_message(&response.leader_id, encrypted_response).await.map_err(|e| HybridConsensusError::NetworkError(e.to_string()))?;
        Ok(())
    }

    async fn handle_append_entries_response(&self, response: AppendEntriesResponse) -> Result<()> {
        self.ccr_layer.handle_append_entries_response(response).await;
        Ok(())
    }

    pub async fn propose(&self, data: Vec<u8>) -> Result<()> {
        let encrypted_data = self.crypto_manager.encrypt(&data).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
        self.ccc_layer.propose(encrypted_data).await.map_err(|e| HybridConsensusError::CCCError(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest_committed_state(&self) -> Result<Vec<u8>> {
        let encrypted_state = self.ccc_layer.get_latest_committed_state().await.map_err(|e| HybridConsensusError::CCCError(e.to_string()))?;
        let decrypted_state = self.crypto_manager.decrypt(&encrypted_state).map_err(|e| HybridConsensusError::CryptoError(e.to_string()))?;
        Ok(decrypted_state)
    }
}