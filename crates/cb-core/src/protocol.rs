//! Sync protocol message types and serialization

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::clipboard::ClipboardContent;
use crate::crypto::{self, Key};
use crate::error::CryptoError;

/// Protocol messages for clipboard sync
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    /// Clipboard text data (plaintext)
    #[serde(rename = "clipboard")]
    Clipboard { text: String, timestamp: u64 },

    /// Clipboard image data (base64-encoded PNG)
    #[serde(rename = "image")]
    Image { data: String, timestamp: u64 },

    /// Encrypted clipboard data (text or image)
    #[serde(rename = "encrypted")]
    Encrypted {
        data: String,
        content_type: ContentType,
        timestamp: u64,
    },

    /// Acknowledgment from receiver to sender
    #[serde(rename = "ack")]
    Ack,
}

/// Content type for encrypted messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Image,
}

impl Message {
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Create a new clipboard text message with current timestamp
    pub fn clipboard(text: String) -> Self {
        Self::Clipboard {
            text,
            timestamp: Self::current_timestamp(),
        }
    }

    /// Create a new clipboard image message (base64-encoded PNG)
    pub fn image(data: Vec<u8>) -> Self {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
        Self::Image {
            data: encoded,
            timestamp: Self::current_timestamp(),
        }
    }

    /// Create a message from clipboard content
    pub fn from_content(content: ClipboardContent) -> Self {
        match content {
            ClipboardContent::Text(text) => Self::clipboard(text),
            ClipboardContent::Image(data) => Self::image(data),
        }
    }

    /// Create an encrypted text message
    pub fn encrypted_text(text: &str, key: &Key) -> Result<Self, CryptoError> {
        let data = crypto::encrypt_string(key, text)?;
        Ok(Self::Encrypted {
            data,
            content_type: ContentType::Text,
            timestamp: Self::current_timestamp(),
        })
    }

    /// Create an encrypted image message
    pub fn encrypted_image(image_data: &[u8], key: &Key) -> Result<Self, CryptoError> {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(image_data);
        let data = crypto::encrypt_string(key, &b64)?;
        Ok(Self::Encrypted {
            data,
            content_type: ContentType::Image,
            timestamp: Self::current_timestamp(),
        })
    }

    /// Create an encrypted message from clipboard content
    pub fn encrypted_content(content: &ClipboardContent, key: &Key) -> Result<Self, CryptoError> {
        match content {
            ClipboardContent::Text(text) => Self::encrypted_text(text, key),
            ClipboardContent::Image(data) => Self::encrypted_image(data, key),
        }
    }

    /// Decrypt an encrypted message to clipboard content
    pub fn decrypt_content(&self, key: &Key) -> Result<ClipboardContent, CryptoError> {
        match self {
            Self::Encrypted {
                data, content_type, ..
            } => {
                let decrypted = crypto::decrypt_string(key, data)?;
                match content_type {
                    ContentType::Text => Ok(ClipboardContent::Text(decrypted)),
                    ContentType::Image => {
                        use base64::Engine;
                        let image_data = base64::engine::general_purpose::STANDARD
                            .decode(&decrypted)
                            .map_err(|_| CryptoError::DecryptionFailed)?;
                        Ok(ClipboardContent::Image(image_data))
                    }
                }
            }
            Self::Clipboard { text, .. } => Ok(ClipboardContent::Text(text.clone())),
            Self::Image { data, .. } => {
                use base64::Engine;
                let image_data = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| CryptoError::DecryptionFailed)?;
                Ok(ClipboardContent::Image(image_data))
            }
            Self::Ack => Err(CryptoError::DecryptionFailed),
        }
    }

    /// Decrypt an encrypted message (text only, for backward compatibility)
    pub fn decrypt(&self, key: &Key) -> Result<String, CryptoError> {
        match self.decrypt_content(key)? {
            ClipboardContent::Text(text) => Ok(text),
            ClipboardContent::Image(_) => Err(CryptoError::DecryptionFailed),
        }
    }

    /// Get content from unencrypted message
    pub fn content(&self) -> Result<ClipboardContent, CryptoError> {
        match self {
            Self::Clipboard { text, .. } => Ok(ClipboardContent::Text(text.clone())),
            Self::Image { data, .. } => {
                use base64::Engine;
                let image_data = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| CryptoError::DecryptionFailed)?;
                Ok(ClipboardContent::Image(image_data))
            }
            Self::Encrypted { .. } => Err(CryptoError::DecryptionFailed),
            Self::Ack => Err(CryptoError::DecryptionFailed),
        }
    }

    /// Create an acknowledgment message
    pub fn ack() -> Self {
        Self::Ack
    }

    /// Serialize message to JSON bytes with newline delimiter
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Deserialize message from JSON bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Check if this is an encrypted message
    pub fn is_encrypted(&self) -> bool {
        matches!(self, Self::Encrypted { .. })
    }

    /// Check if this is an image message
    pub fn is_image(&self) -> bool {
        matches!(
            self,
            Self::Image { .. }
                | Self::Encrypted {
                    content_type: ContentType::Image,
                    ..
                }
        )
    }

    /// Get the content type
    pub fn content_type(&self) -> Option<ContentType> {
        match self {
            Self::Clipboard { .. } => Some(ContentType::Text),
            Self::Image { .. } => Some(ContentType::Image),
            Self::Encrypted { content_type, .. } => Some(*content_type),
            Self::Ack => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_key;

    #[test]
    fn clipboard_message_serialization() {
        let msg = Message::Clipboard {
            text: "hello".to_string(),
            timestamp: 1234567890,
        };
        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        match parsed {
            Message::Clipboard { text, timestamp } => {
                assert_eq!(text, "hello");
                assert_eq!(timestamp, 1234567890);
            }
            _ => panic!("expected Clipboard message"),
        }
    }

    #[test]
    fn ack_message_serialization() {
        let msg = Message::ack();
        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        assert!(matches!(parsed, Message::Ack));
    }

    #[test]
    fn encrypted_text_roundtrip() {
        let key = generate_key();
        let plaintext = "secret message";

        let msg = Message::encrypted_text(plaintext, &key).unwrap();
        assert!(msg.is_encrypted());
        assert!(!msg.is_image());

        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        let decrypted = parsed.decrypt(&key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypted_image_roundtrip() {
        let key = generate_key();
        let image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header

        let msg = Message::encrypted_image(&image_data, &key).unwrap();
        assert!(msg.is_encrypted());
        assert!(msg.is_image());

        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        let content = parsed.decrypt_content(&key).unwrap();
        assert_eq!(content, ClipboardContent::Image(image_data));
    }

    #[test]
    fn image_message_roundtrip() {
        let image_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]; // PNG header

        let msg = Message::image(image_data.clone());
        assert!(msg.is_image());
        assert!(!msg.is_encrypted());

        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        let content = parsed.content().unwrap();
        assert_eq!(content, ClipboardContent::Image(image_data));
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1 = generate_key();
        let key2 = generate_key();

        let msg = Message::encrypted_text("secret", &key1).unwrap();
        let result = msg.decrypt(&key2);

        assert!(result.is_err());
    }

    #[test]
    fn content_from_text() {
        let content = ClipboardContent::Text("hello".to_string());
        let msg = Message::from_content(content.clone());

        assert!(!msg.is_image());
        assert_eq!(msg.content().unwrap(), content);
    }

    #[test]
    fn content_from_image() {
        let content = ClipboardContent::Image(vec![1, 2, 3, 4]);
        let msg = Message::from_content(content.clone());

        assert!(msg.is_image());
        assert_eq!(msg.content().unwrap(), content);
    }
}
