// src/lib.rs
pub mod node;
pub mod ccr;
pub mod ccc;
pub mod network;
pub mod sharding;
pub mod crypto;
pub mod metrics;

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
}

pub type Result<T> = std::result::Result<T, HybridConsensusError>;
