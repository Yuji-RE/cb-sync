//! Error types for cb-core

use thiserror::Error;

/// Errors that can occur during clipboard operations
#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard is empty")]
    Empty,

    #[error("clipboard command not found: {0}")]
    CommandNotFound(String),

    #[error("clipboard command failed: {0}")]
    CommandFailed(String),

    #[error("invalid utf-8 in clipboard content")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors that can occur during sync operations
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("connection failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("message serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("connection timed out")]
    Timeout,

    #[error("unexpected message type")]
    UnexpectedMessage,

    #[error("encryption/decryption failed: {0}")]
    Crypto(#[from] CryptoError),
}

/// Errors that can occur during crypto operations
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid encryption key")]
    InvalidKey,

    #[error("encryption failed")]
    EncryptionFailed,

    #[error("decryption failed")]
    DecryptionFailed,
}
