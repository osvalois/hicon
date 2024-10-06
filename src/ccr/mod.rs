mod raft;
mod log;

use self::{raft::RaftConsensus, log::Log};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct CCRLayer {
    raft: Arc<RwLock<RaftConsensus>>,
    log: Arc<RwLock<Log>>,
}

impl CCRLayer {
    pub fn new() -> Self {
        CCRLayer {
            raft: Arc::new(RwLock::new(RaftConsensus::new())),
            log: Arc::new(RwLock::new(Log::new())),
        }
    }

    pub async fn should_become_candidate(&self) -> bool {
        self.raft.read().await.should_become_candidate()
    }

    pub async fn election_timeout_elapsed(&self) -> bool {
        self.raft.read().await.election_timeout_elapsed()
    }

    // Implement other CCR layer methods
}
