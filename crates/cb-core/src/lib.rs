//! cb-core: Core library for clipboard sync
//!
//! This crate provides:
//! - Clipboard abstraction layer for cross-platform support
//! - Sync protocol for P2P clipboard sharing
//! - Encryption utilities (ChaCha20-Poly1305)
//!
//! # Clipboard Example
//!
//! ```no_run
//! use cb_core::clipboard::{create_clipboard, Clipboard};
//!
//! let clipboard = create_clipboard();
//!
//! // Write to clipboard
//! clipboard.write_text("Hello, world!").unwrap();
//!
//! // Read from clipboard
//! let text = clipboard.read_text().unwrap();
//! println!("Clipboard: {}", text);
//! ```
//!
//! # Sync Example
//!
//! ```no_run
//! use cb_core::sync;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Send clipboard to another device
//!     let addr: SocketAddr = "<TARGET_IP>:34812".parse().unwrap();
//!     sync::send(addr, "Hello from sender!".to_string()).await.unwrap();
//!
//!     // Or receive from another device
//!     let listen_addr: SocketAddr = "0.0.0.0:34812".parse().unwrap();
//!     let text = sync::receive_once(listen_addr).await.unwrap();
//!     println!("Received: {}", text);
//! }
//! ```

pub mod clipboard;
pub mod config;
pub mod crypto;
pub mod error;
pub mod protocol;
pub mod sync;

// Re-export commonly used types
#[cfg(target_os = "windows")]
pub use clipboard::WindowsClipboard;
pub use clipboard::{
    Clipboard, ClipboardContent, DisplayServer, WaylandClipboard, X11Clipboard, create_clipboard,
    detect_display_server,
};
pub use config::{Config, ConfigError, EncryptionConfig, GeneralConfig, TargetConfig};
pub use crypto::{generate_key, key_from_base64, key_from_password, key_to_base64};
pub use error::{ClipboardError, CryptoError, SyncError};
pub use sync::DEFAULT_PORT;
