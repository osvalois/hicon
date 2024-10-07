// src/ccc/pbft.rs

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest as Sha256Digest};
use crate::{HybridConsensusError, network::NetworkManager, storage::StateStorage};
use std::sync::Arc;

pub use super::NodeId;
type SequenceNumber = u64;
type Digest = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PBFTMessage {
    Request(Request),
    PrePrepare(PrePrepare),
    Prepare(Prepare),
    Commit(Commit),
    Reply(Reply),
    ViewChange(ViewChange),
    NewView(NewView),
    Checkpoint(Checkpoint),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub timestamp: u64,
    pub client_id: NodeId,
    pub operation: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrePrepare {
    pub view: u64,
    pub sequence: SequenceNumber,
    pub digest: Digest,
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepare {
    pub view: u64,
    pub sequence: SequenceNumber,
    pub digest: Digest,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub view: u64,
    pub sequence: SequenceNumber,
    pub digest: Digest,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub view: u64,
    pub timestamp: u64,
    pub client_id: NodeId,
    pub node_id: NodeId,
    pub result: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChange {
    pub new_view: u64,
    pub last_sequence: SequenceNumber,
    pub node_id: NodeId,
    pub checkpoints: Vec<Checkpoint>,
    pub prepared_proofs: Vec<PreparedProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewView {
    pub new_view: u64,
    pub view_change_proofs: Vec<ViewChange>,
    pub new_sequence: SequenceNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub sequence: SequenceNumber,
    pub digest: Digest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedProof {
    pub pre_prepare: PrePrepare,
    pub prepares: Vec<Prepare>,
}

pub struct PBFTConsensus {
    node_id: NodeId,
    view: u64,
    sequence: SequenceNumber,
    log: HashMap<SequenceNumber, PBFTMessage>,
    prepared: HashMap<SequenceNumber, PreparedProof>,
    committed: HashMap<SequenceNumber, Digest>,
    checkpoints: Vec<Checkpoint>,
    view_change_timeout: Duration,
    last_checkpoint_time: Instant,
    checkpoint_interval: Duration,
    f: usize,
    network: Arc<NetworkManager>,
    storage: Arc<StateStorage>,
    client_table: HashMap<NodeId, Reply>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Invalid message")]
    InvalidMessage,
    #[error("Invalid digest")]
    InvalidDigest,
    #[error("Invalid view change")]
    InvalidViewChange,
    #[error("Invalid new view")]
    InvalidNewView,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Storage error: {0}")]
    StorageError(String),
}

impl From<ConsensusError> for HybridConsensusError {
    fn from(error: ConsensusError) -> Self {
        HybridConsensusError::ConsensusError(error.to_string())
    }
}

impl PBFTConsensus {
    pub fn new(
        node_id: NodeId,
        f: usize,
        network: Arc<NetworkManager>,
        storage: Arc<StateStorage>,
    ) -> Self {
        PBFTConsensus {
            node_id,
            view: 0,
            sequence: 0,
            log: HashMap::new(),
            prepared: HashMap::new(),
            committed: HashMap::new(),
            checkpoints: Vec::new(),
            view_change_timeout: Duration::from_secs(5),
            last_checkpoint_time: Instant::now(),
            checkpoint_interval: Duration::from_secs(100),
            f,
            network,
            storage,
            client_table: HashMap::new(),
        }
    }

    pub async fn handle_message(&mut self, message: PBFTMessage) -> Result<(), ConsensusError> {
        match message {
            PBFTMessage::Request(req) => self.handle_request(req).await,
            PBFTMessage::PrePrepare(pp) => self.handle_pre_prepare(pp).await,
            PBFTMessage::Prepare(p) => self.handle_prepare(p).await,
            PBFTMessage::Commit(c) => self.handle_commit(c).await,
            PBFTMessage::Reply(_) => Ok(()), // Replies are handled by clients
            PBFTMessage::ViewChange(vc) => self.handle_view_change(vc).await,
            PBFTMessage::NewView(nv) => self.handle_new_view(nv).await,
            PBFTMessage::Checkpoint(cp) => self.handle_checkpoint(cp).await,
        }
    }

    async fn handle_request(&mut self, req: Request) -> Result<(), ConsensusError> {
        if self.is_primary() {
            let digest = self.compute_digest(&req);
            let pre_prepare = PrePrepare {
                view: self.view,
                sequence: self.sequence,
                digest,
                request: req,
            };
            self.broadcast(PBFTMessage::PrePrepare(pre_prepare)).await?;
            self.sequence += 1;
        }
        Ok(())
    }

    async fn handle_pre_prepare(&mut self, pp: PrePrepare) -> Result<(), ConsensusError> {
        if pp.view != self.view || pp.sequence <= self.last_stable_checkpoint() {
            return Err(ConsensusError::InvalidMessage);
        }

        let computed_digest = self.compute_digest(&pp.request);
        if computed_digest != pp.digest {
            return Err(ConsensusError::InvalidDigest);
        }

        self.log.insert(pp.sequence, PBFTMessage::PrePrepare(pp.clone()));

        let prepare = Prepare {
            view: self.view,
            sequence: pp.sequence,
            digest: pp.digest,
            node_id: self.node_id.clone(),
        };

        self.broadcast(PBFTMessage::Prepare(prepare)).await?;
        Ok(())
    }

    async fn handle_prepare(&mut self, p: Prepare) -> Result<(), ConsensusError> {
        if p.view != self.view || p.sequence <= self.last_stable_checkpoint() {
            return Err(ConsensusError::InvalidMessage);
        }

        if let Some(PBFTMessage::PrePrepare(pp)) = self.log.get(&p.sequence) {
            if pp.digest != p.digest {
                return Err(ConsensusError::InvalidDigest);
            }

            if let Some(proof) = self.prepared.get_mut(&p.sequence) {
                proof.prepares.push(p.clone());
            } else {
                let proof = PreparedProof {
                    pre_prepare: pp.clone(),
                    prepares: vec![p.clone()],
                };
                self.prepared.insert(p.sequence, proof);
            }

            if self.is_prepared(&p.sequence) {
                let commit = Commit {
                    view: self.view,
                    sequence: p.sequence,
                    digest: p.digest,
                    node_id: self.node_id.clone(),
                };
                self.broadcast(PBFTMessage::Commit(commit)).await?;
            }
        }
        Ok(())
    }

    async fn handle_commit(&mut self, c: Commit) -> Result<(), ConsensusError> {
        if c.view != self.view || c.sequence <= self.last_stable_checkpoint() {
            return Err(ConsensusError::InvalidMessage);
        }

        if let Some(proof) = self.prepared.get(&c.sequence) {
            if proof.pre_prepare.digest != c.digest {
                return Err(ConsensusError::InvalidDigest);
            }

            self.committed.insert(c.sequence, c.digest);

            if self.is_committed(&c.sequence) {
                let result = self.execute_request(&proof.pre_prepare.request).await?;
                let reply = Reply {
                    view: self.view,
                    timestamp: proof.pre_prepare.request.timestamp,
                    client_id: proof.pre_prepare.request.client_id.clone(),
                    node_id: self.node_id.clone(),
                    result,
                };
                self.send_to_client(reply).await?;

                if Instant::now().duration_since(self.last_checkpoint_time) >= self.checkpoint_interval {
                    self.create_checkpoint().await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_view_change(&mut self, vc: ViewChange) -> Result<(), ConsensusError> {
        if vc.new_view <= self.view {
            return Err(ConsensusError::InvalidViewChange);
        }
        // Implement complete view change logic here
        self.view = vc.new_view;
        // Reset timers, update state, etc.
        Ok(())
    }

    async fn handle_new_view(&mut self, nv: NewView) -> Result<(), ConsensusError> {
        if nv.new_view <= self.view {
            return Err(ConsensusError::InvalidNewView);
        }
        // Implement complete new view logic here
        self.view = nv.new_view;
        self.sequence = nv.new_sequence;
        // Process any pending requests, update state, etc.
        Ok(())
    }

    async fn handle_checkpoint(&mut self, cp: Checkpoint) -> Result<(), ConsensusError> {
        // Implement checkpoint handling logic
        // Verify the checkpoint, update stable checkpoints, garbage collect, etc.
        Ok(())
    }

    async fn create_checkpoint(&mut self) -> Result<(), ConsensusError> {
        let checkpoint = Checkpoint {
            sequence: self.sequence,
            digest: self.compute_state_digest().await?,
        };
        self.checkpoints.push(checkpoint.clone());
        self.last_checkpoint_time = Instant::now();

        // Broadcast checkpoint message
        self.broadcast(PBFTMessage::Checkpoint(checkpoint)).await?;

        // Remove old messages from the log
        self.garbage_collect(self.last_stable_checkpoint());

        Ok(())
    }

    fn is_primary(&self) -> bool {
        (self.view % (self.f * 2 + 1) as u64) == self.node_id.parse::<u64>().unwrap_or(0)
    }

    fn is_prepared(&self, sequence: &SequenceNumber) -> bool {
        if let Some(proof) = self.prepared.get(sequence) {
            proof.prepares.len() >= 2 * self.f
        } else {
            false
        }
    }

    fn is_committed(&self, sequence: &SequenceNumber) -> bool {
        self.committed.get(sequence).is_some()
    }

    fn last_stable_checkpoint(&self) -> SequenceNumber {
        self.checkpoints.last().map_or(0, |c| c.sequence)
    }

    fn compute_digest(&self, request: &Request) -> Digest {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(request).unwrap().as_bytes());
        hasher.finalize().into()
    }

    async fn compute_state_digest(&self) -> Result<Digest, ConsensusError> {
        let state = self.storage.get_state().await
            .map_err(|e| ConsensusError::StorageError(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&state);
        Ok(hasher.finalize().into())
    }

    fn garbage_collect(&mut self, stable_sequence: SequenceNumber) {
        self.log.retain(|&k, _| k > stable_sequence);
        self.prepared.retain(|&k, _| k > stable_sequence);
        self.committed.retain(|&k, _| k > stable_sequence);
    }

    async fn broadcast(&self, message: PBFTMessage) -> Result<(), ConsensusError> {
        let serialized = serde_json::to_vec(&message)
            .map_err(|e| ConsensusError::NetworkError(e.to_string()))?;
        
        for peer in self.network.get_peers().await
            .map_err(|e| ConsensusError::NetworkError(e.to_string()))? {
            self.network.send_message(&peer, &serialized).await
                .map_err(|e| ConsensusError::NetworkError(e.to_string()))?;
        }
        
        Ok(())
    }

    async fn send_to_client(&self, reply: Reply) -> Result<(), ConsensusError> {
        let serialized = serde_json::to_vec(&PBFTMessage::Reply(reply.clone()))
            .map_err(|e| ConsensusError::NetworkError(e.to_string()))?;
        
        self.network.send_message(&reply.client_id, &serialized).await
            .map_err(|e| ConsensusError::NetworkError(e.to_string()))?;
        
        Ok(())
    }

    async fn execute_request(&self, request: &Request) -> Result<Vec<u8>, ConsensusError> {
        // In a real implementation, this would execute the request and return the result
        // For this example, we'll just store the operation in our state
        self.storage.update_state(&request.operation).await
            .map_err(|e| ConsensusError::StorageError(e.to_string()))?;
        Ok(request.operation.clone())
    }
    pub async fn get_latest_committed_state(&self) -> Result<Vec<u8>, ConsensusError> {
        self.storage.get_state().await
            .map_err(|e| ConsensusError::StorageError(e.to_string()))
    }

    async fn start_view_change(&mut self) -> Result<(), ConsensusError> {
        self.view += 1;
        let view_change = ViewChange {
            new_view: self.view,
            last_sequence: self.sequence,
            node_id: self.node_id.clone(),
            checkpoints: self.checkpoints.clone(),
            prepared_proofs: self.prepared.values().cloned().collect(),
        };
        self.broadcast(PBFTMessage::ViewChange(view_change)).await
    }

    async fn process_view_change(&mut self, view_changes: Vec<ViewChange>) -> Result<(), ConsensusError> {
        if view_changes.len() < 2 * self.f + 1 {
            return Err(ConsensusError::InvalidViewChange);
        }

        let new_view = view_changes[0].new_view;
        if !view_changes.iter().all(|vc| vc.new_view == new_view) {
            return Err(ConsensusError::InvalidViewChange);
        }

        // Determine the highest last_sequence and update the sequence number
        let max_sequence = view_changes.iter()
            .map(|vc| vc.last_sequence)
            .max()
            .unwrap_or(self.sequence);

        self.view = new_view;
        self.sequence = max_sequence + 1;

        // Create and broadcast NewView message
        let new_view_msg = NewView {
            new_view,
            view_change_proofs: view_changes,
            new_sequence: self.sequence,
        };
        self.broadcast(PBFTMessage::NewView(new_view_msg)).await
    }

    fn update_client_table(&mut self, reply: &Reply) {
        self.client_table.insert(reply.client_id.clone(), reply.clone());
    }

    fn get_client_last_reply(&self, client_id: &NodeId) -> Option<&Reply> {
        self.client_table.get(client_id)
    }

    pub async fn run(&mut self) -> Result<(), ConsensusError> {
        loop {
            tokio::select! {
                message = self.network.receive() => {
                    if let Ok(message) = message {
                        if let Err(e) = self.handle_message(message).await {
                            tracing::error!("Error handling message: {:?}", e);
                        }
                    }
                }
                _ = tokio::time::sleep(self.view_change_timeout) => {
                    if let Err(e) = self.start_view_change().await {
                        tracing::error!("Error starting view change: {:?}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use std::sync::Arc;

    struct MockNetworkManager;
    struct MockStateStorage;

    // Implement mock NetworkManager and StateStorage...

    #[tokio::test]
    async fn test_pbft_consensus() {
        let network = Arc::new(MockNetworkManager);
        let storage = Arc::new(MockStateStorage);
        let mut pbft = PBFTConsensus::new("node1".to_string(), 1, network, storage);

        // Test request handling
        let request = Request {
            timestamp: 1,
            client_id: "client1".to_string(),
            operation: vec![1, 2, 3],
        };
        pbft.handle_request(request.clone()).await.unwrap();

        // Test pre-prepare handling
        let pre_prepare = PrePrepare {
            view: 0,
            sequence: 1,
            digest: pbft.compute_digest(&request),
            request: request.clone(),
        };
        pbft.handle_pre_prepare(pre_prepare.clone()).await.unwrap();

        // Test prepare handling
        let prepare = Prepare {
            view: 0,
            sequence: 1,
            digest: pre_prepare.digest,
            node_id: "node2".to_string(),
        };
        pbft.handle_prepare(prepare).await.unwrap();

        // Test commit handling
        let commit = Commit {
            view: 0,
            sequence: 1,
            digest: pre_prepare.digest,
            node_id: "node2".to_string(),
        };
        pbft.handle_commit(commit).await.unwrap();

        // Assert that the request has been executed and a reply sent
        assert!(pbft.committed.contains_key(&1));
    }
}