mod pbft;

use self::pbft::PBFTConsensus;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct CCCLayer {
    pbft: Arc<RwLock<PBFTConsensus>>,
}

impl CCCLayer {
    pub fn new() -> Self {
        CCCLayer {
            pbft: Arc::new(RwLock::new(PBFTConsensus::new())),
        }
    }

    // Implement CCC layer methods
}
