use std::time::{Duration, Instant};
use rand::Rng;

pub struct RaftConsensus {
    last_heartbeat: Instant,
    election_timeout: Duration,
}

impl RaftConsensus {
    pub fn new() -> Self {
        RaftConsensus {
            last_heartbeat: Instant::now(),
            election_timeout: Duration::from_millis(rand::thread_rng().gen_range(150..300)),
        }
    }

    pub fn should_become_candidate(&self) -> bool {
        self.last_heartbeat.elapsed() > self.election_timeout
    }

    pub fn election_timeout_elapsed(&self) -> bool {
        self.last_heartbeat.elapsed() > self.election_timeout
    }

    // Implement other Raft-specific methods
}
