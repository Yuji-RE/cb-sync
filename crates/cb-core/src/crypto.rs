//! Encryption utilities using ChaCha20-Poly1305
//!
//! Provides symmetric encryption for clipboard data using a shared secret.
//! Key derivation uses Argon2id for password-based keys.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use rand::RngCore;

use crate::error::CryptoError;

/// Result type for crypto operations
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Encryption key (256 bits)
pub type Key = [u8; 32];

/// Application-specific salt for password-based key derivation
/// This provides domain separation; for stronger security, use random salt per message
const APP_SALT: &[u8; 16] = b"cb-sync-v1-salt!";

/// Generate a new random encryption key
pub fn generate_key() -> Key {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Derive a key from a password using Argon2id
///
/// Uses application-specific fixed salt for simplicity.
/// Argon2id parameters are tuned for interactive use (fast but still secure).
pub fn key_from_password(password: &str) -> Key {
    // Argon2id with moderate parameters for interactive use
    // m=19456 KiB (19 MiB), t=2 iterations, p=1 parallelism
    let params = Params::new(19456, 2, 1, Some(32)).expect("valid Argon2 params");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), APP_SALT, &mut key)
        .expect("Argon2 hashing should not fail with valid params");

    key
}

/// Encode a key to base64 for display/storage
pub fn key_to_base64(key: &Key) -> String {
    BASE64.encode(key)
}

/// Decode a key from base64
pub fn key_from_base64(encoded: &str) -> Result<Key> {
    let bytes = BASE64
        .decode(encoded)
        .map_err(|_| CryptoError::InvalidKey)?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidKey);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Encrypt data with the given key
///
/// Returns base64-encoded ciphertext with nonce prepended
pub fn encrypt(key: &Key, plaintext: &[u8]) -> Result<String> {
    let cipher = ChaCha20Poly1305::new(key.into());

    // Generate random nonce (12 bytes for ChaCha20-Poly1305)
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&result))
}

/// Decrypt base64-encoded ciphertext with the given key
pub fn decrypt(key: &Key, encrypted: &str) -> Result<Vec<u8>> {
    let data = BASE64
        .decode(encrypted)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    if data.len() < 12 {
        return Err(CryptoError::DecryptionFailed);
    }

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(key.into());

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Encrypt a string
pub fn encrypt_string(key: &Key, plaintext: &str) -> Result<String> {
    encrypt(key, plaintext.as_bytes())
}

/// Decrypt to a string
pub fn decrypt_string(key: &Key, encrypted: &str) -> Result<String> {
    let bytes = decrypt(key, encrypted)?;
    String::from_utf8(bytes).map_err(|_| CryptoError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = "Hello, World!";

        let encrypted = encrypt_string(&key, plaintext).unwrap();
        let decrypted = decrypt_string(&key, &encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn key_from_password_is_deterministic() {
        let key1 = key_from_password("my-secret-password");
        let key2 = key_from_password("my-secret-password");

        assert_eq!(key1, key2);
    }

    #[test]
    fn key_base64_roundtrip() {
        let key = generate_key();
        let encoded = key_to_base64(&key);
        let decoded = key_from_base64(&encoded).unwrap();

        assert_eq!(key, decoded);
    }

    #[test]
    fn different_keys_produce_different_ciphertext() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = "Hello";

        let enc1 = encrypt_string(&key1, plaintext).unwrap();
        let enc2 = encrypt_string(&key2, plaintext).unwrap();

        assert_ne!(enc1, enc2);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = "Hello";

        let encrypted = encrypt_string(&key1, plaintext).unwrap();
        let result = decrypt_string(&key2, &encrypted);

        assert!(result.is_err());
    }
}
