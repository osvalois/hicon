// src/crypto/post_quantum.rs
use pqcrypto_kyber::kyber768;
use pqcrypto_traits::kem::{PublicKey, SecretKey, SharedSecret};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;

pub struct PostQuantumCrypto {
    public_key: kyber768::PublicKey,
    secret_key: kyber768::SecretKey,
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Key generation error: {0}")]
    KeyGenerationError(String),
}

impl PostQuantumCrypto {
    pub fn new() -> Result<Self, CryptoError> {
        let (public_key, secret_key) = kyber768::keypair();
        Ok(PostQuantumCrypto {
            public_key,
            secret_key,
        })
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        // Generate a random nonce
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        // Encapsulate a shared secret using Kyber
        let (ciphertext, shared_secret) = kyber768::encapsulate(&self.public_key);

        // Use the shared secret to create an AES key
        let key = Key::<Aes256Gcm>::from_slice(shared_secret.as_ref());
        let cipher = Aes256Gcm::new(key);

        // Encrypt the data using AES-GCM
        let encrypted_data = cipher
            .encrypt(Nonce::from_slice(&nonce), data)
            .map_err(|e| CryptoError::EncryptionError(e.to_string()))?;

        // Combine nonce, Kyber ciphertext, and encrypted data
        let mut result = Vec::new();
        result.extend_from_slice(&nonce);
        result.extend_from_slice(ciphertext.as_ref());
        result.extend_from_slice(&encrypted_data);

        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if data.len() < 12 + kyber768::CIPHERTEXT_SIZE {
            return Err(CryptoError::DecryptionError("Invalid ciphertext length".to_string()));
        }

        // Extract nonce, Kyber ciphertext, and encrypted data
        let nonce = &data[..12];
        let kyber_ciphertext = kyber768::Ciphertext::from_slice(&data[12..12 + kyber768::CIPHERTEXT_SIZE])
            .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;
        let encrypted_data = &data[12 + kyber768::CIPHERTEXT_SIZE..];

        // Decapsulate the shared secret
        let shared_secret = kyber768::decapsulate(&kyber_ciphertext, &self.secret_key);

        // Use the shared secret to create an AES key
        let key = Key::<Aes256Gcm>::from_slice(shared_secret.as_ref());
        let cipher = Aes256Gcm::new(key);

        // Decrypt the data using AES-GCM
        let decrypted_data = cipher
            .decrypt(Nonce::from_slice(nonce), encrypted_data)
            .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

        Ok(decrypted_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let crypto = PostQuantumCrypto::new().unwrap();
        let original_data = b"Hello, post-quantum world!";

        let encrypted = crypto.encrypt(original_data).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();

        assert_eq!(original_data.to_vec(), decrypted);
    }
}