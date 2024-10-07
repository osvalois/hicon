// src/lib.rs
pub mod ccr;
pub mod ccc;
pub mod network;
pub mod sharding;
pub mod crypto;
pub mod metrics;
pub mod consensus;
pub mod node;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HybridConsensusError {
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Consensus error: {0}")]
    ConsensusError(String),
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("CCR error: {0}")]
    CCRError(String),
    #[error("CCC error: {0}")]
    CCCError(String),
}

pub type Result<T> = std::result::Result<T, HybridConsensusError>;
