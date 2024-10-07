// src/ccc/mod.rs

mod pbft;

use self::pbft::{PBFTConsensus, PBFTMessage, ConsensusError};
use crate::HybridConsensusError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use serde::{Serialize, Deserialize};
use chrono::Utc;

pub type NodeId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CCCMessage {
    PBFT(PBFTMessage),
    LayerCommunication(LayerCommunicationMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCommunicationMessage {
    from: NodeId,
    to: NodeId,
    content: Vec<u8>,
}

pub struct CCCLayer {
    node_id: NodeId,
    pbft: Arc<RwLock<PBFTConsensus>>,
    communication_costs: HashMap<(NodeId, NodeId), f64>,
    network_sender: mpsc::Sender<(NodeId, CCCMessage)>,
    layer_receiver: mpsc::Receiver<CCCMessage>,
}

#[derive(Debug, thiserror::Error)]
pub enum CCCError {
    #[error("Consensus error: {0}")]
    ConsensusError(#[from] ConsensusError),
    #[error("Network error")]
    NetworkError,
    #[error("Layer communication error: {0}")]
    LayerCommunicationError(String),
}

impl From<CCCError> for HybridConsensusError {
    fn from(error: CCCError) -> Self {
        match error {
            CCCError::ConsensusError(e) => HybridConsensusError::ConsensusError(e.to_string()),
            CCCError::NetworkError => HybridConsensusError::NetworkError("CCC Network Error".to_string()),
            CCCError::LayerCommunicationError(e) => HybridConsensusError::CCCError(e),
        }
    }
}

impl CCCLayer {
    pub fn new(
        node_id: NodeId,
        f: usize,
        network_sender: mpsc::Sender<(NodeId, CCCMessage)>,
        layer_receiver: mpsc::Receiver<CCCMessage>,
    ) -> Self {
        let (pbft_sender, _) = mpsc::channel(1000);
        CCCLayer {
            node_id: node_id.clone(),
            pbft: Arc::new(RwLock::new(PBFTConsensus::new(node_id, f, pbft_sender))),
            communication_costs: HashMap::new(),
            network_sender,
            layer_receiver,
        }
    }

    pub async fn run(&mut self) -> Result<(), CCCError> {
        while let Some(message) = self.layer_receiver.recv().await {
            self.handle_message(message).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: CCCMessage) -> Result<(), CCCError> {
        match message {
            CCCMessage::PBFT(pbft_msg) => {
                let mut pbft = self.pbft.write().await;
                pbft.handle_message(pbft_msg).await?;
            }
            CCCMessage::LayerCommunication(layer_msg) => {
                self.handle_layer_communication(layer_msg).await?;
            }
        }
        Ok(())
    }

    async fn handle_layer_communication(&mut self, message: LayerCommunicationMessage) -> Result<(), CCCError> {
        println!("Handling layer communication from {} to {}", message.from, message.to);
        // Implement actual layer communication logic here
        Ok(())
    }

    pub async fn send_message(&self, to: NodeId, message: CCCMessage) -> Result<(), CCCError> {
        self.network_sender.send((to, message)).await.map_err(|_| CCCError::NetworkError)?;
        Ok(())
    }

    pub async fn optimize_communication(&mut self) -> Result<(), CCCError> {
        println!("Optimizing communication for node {}", self.node_id);
        // Implement communication optimization logic here
        Ok(())
    }

    pub async fn update_communication_cost(&mut self, from: NodeId, to: NodeId, cost: f64) {
        self.communication_costs.insert((from, to), cost);
    }

    pub fn get_optimal_path(&self, from: NodeId, to: NodeId) -> Vec<NodeId> {
        // Implement optimal path finding algorithm here
        // For now, just return direct path
        vec![from, to]
    }

    async fn broadcast(&self, message: CCCMessage) -> Result<(), CCCError> {
        for (node_pair, _) in &self.communication_costs {
            if node_pair.0 == self.node_id {
                self.send_message(node_pair.1.clone(), message.clone()).await?;
            }
        }
        Ok(())
    }

    // Public API
    pub async fn propose(&self, request: Vec<u8>) -> Result<(), CCCError> {
        let pbft_message = PBFTMessage::Request(pbft::Request {
            timestamp: Utc::now().timestamp() as u64,
            client_id: self.node_id.clone(),
            operation: request,
        });
        self.broadcast(CCCMessage::PBFT(pbft_message)).await
    }

    pub async fn get_latest_committed_state(&self) -> Result<Vec<u8>, CCCError> {
        let pbft = self.pbft.read().await;
        Ok(pbft.get_latest_committed_state())
    }
}