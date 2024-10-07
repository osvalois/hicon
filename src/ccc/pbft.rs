// src/ccc/pbft.rs

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest as Sha256Digest};
use crate::HybridConsensusError;

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
    network_sender: mpsc::Sender<(NodeId, PBFTMessage)>,
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
    #[error("Network error")]
    NetworkError,
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
        network_sender: mpsc::Sender<(NodeId, PBFTMessage)>,
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
            network_sender,
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
                let result = self.execute_request(&proof.pre_prepare.request);
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
        // Implement view change logic here
        // For now, we'll just update the view
        self.view = vc.new_view;
        Ok(())
    }

    async fn handle_new_view(&mut self, nv: NewView) -> Result<(), ConsensusError> {
        if nv.new_view <= self.view {
            return Err(ConsensusError::InvalidNewView);
        }
        // Implement new view logic here
        // For now, we'll just update the view and sequence
        self.view = nv.new_view;
        self.sequence = nv.new_sequence;
        Ok(())
    }

    async fn create_checkpoint(&mut self) -> Result<(), ConsensusError> {
        let checkpoint = Checkpoint {
            sequence: self.sequence,
            digest: self.compute_state_digest(),
        };
        self.checkpoints.push(checkpoint.clone());
        self.last_checkpoint_time = Instant::now();

        // Broadcast checkpoint message
        self.broadcast(PBFTMessage::Commit(Commit {
            view: self.view,
            sequence: checkpoint.sequence,
            digest: checkpoint.digest,
            node_id: self.node_id.clone(),
        }))
        .await?;

        // Remove old messages from the log
        self.garbage_collect(checkpoint.sequence);

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

    fn compute_state_digest(&self) -> Digest {
        // In a real implementation, this would compute a digest of the current state
        // For now, we'll just return a dummy value
        [0; 32]
    }

    fn garbage_collect(&mut self, stable_sequence: SequenceNumber) {
        self.log.retain(|&k, _| k > stable_sequence);
        self.prepared.retain(|&k, _| k > stable_sequence);
        self.committed.retain(|&k, _| k > stable_sequence);
    }

    async fn broadcast(&self, message: PBFTMessage) -> Result<(), ConsensusError> {
        // In a real implementation, this would send the message to all other nodes
        // For now, we'll just print the message
        println!("Broadcasting: {:?}", message);
        Ok(())
    }

    async fn send_to_client(&self, reply: Reply) -> Result<(), ConsensusError> {
        // In a real implementation, this would send the reply to the client
        // For now, we'll just print the reply
        println!("Sending reply to client: {:?}", reply);
        Ok(())
    }

    fn execute_request(&self, request: &Request) -> Vec<u8> {
        // In a real implementation, this would execute the request and return the result
        // For now, we'll just return a dummy value
        vec![0, 1, 2, 3]
    }

    pub fn get_latest_committed_state(&self) -> Vec<u8> {
        // In a real implementation, this would return the latest committed state
        // For now, we'll just return a dummy value
        vec![0, 1, 2, 3]
    }
}