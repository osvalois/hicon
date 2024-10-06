mod post_quantum;

use self::post_quantum::PostQuantumCrypto;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    // Define other crypto-related errors
}

pub struct CryptoManager {
    pq_crypto: PostQuantumCrypto,
}

impl CryptoManager {
    pub fn new() -> Self {
        CryptoManager {
            pq_crypto: PostQuantumCrypto::new(),
        }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.pq_crypto.encrypt(data)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.pq_crypto.decrypt(data)
    }
}
