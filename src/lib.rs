pub mod node;
pub mod ccr;
pub mod ccc;
pub mod network;
pub mod sharding;
pub mod crypto;
pub mod metrics;
pub mod consensus;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HybridConsensusError {
    #[error("Network error: {0}")]
    NetworkError(#[from] network::NetworkError),
    #[error("Consensus error: {0}")]
    ConsensusError(String),
    #[error("Cryptography error: {0}")]
    CryptoError(#[from] crypto::CryptoError),
    #[error("Sharding error: {0}")]
    ShardingError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, HybridConsensusError>;