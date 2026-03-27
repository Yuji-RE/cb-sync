//! Daemon mode for automatic bidirectional clipboard sync
//!
//! The daemon:
//! 1. Polls local clipboard for changes
//! 2. Broadcasts changes to configured peers
//! 3. Listens for incoming sync from peers
//! 4. Writes received content to local clipboard
//!
//! # Security
//!
//! Daemon mode requires encryption. Plaintext sync is not allowed.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::clipboard::{Clipboard, ClipboardContent, create_clipboard};
use crate::config::DaemonConfig;
use crate::crypto::Key;
use crate::protocol::Message;
use crate::sync::{self, CLIPBOARD_TIMEOUT, MAX_MESSAGE_SIZE};

/// Daemon state shared between tasks
struct DaemonState {
    /// Hash of last clipboard content we processed
    last_content_hash: Option<[u8; 32]>,
    /// Hash of content received from peer (for loop prevention)
    last_received_hash: Option<[u8; 32]>,
    /// When we last received content
    last_received_time: Option<Instant>,
}

impl DaemonState {
    fn new() -> Self {
        Self {
            last_content_hash: None,
            last_received_hash: None,
            last_received_time: None,
        }
    }
}

/// Compute SHA-256 hash of clipboard content
fn hash_content(content: &ClipboardContent) -> [u8; 32] {
    let mut hasher = Sha256::new();
    match content {
        ClipboardContent::Text(text) => {
            hasher.update(b"text:");
            hasher.update(text.as_bytes());
        }
        ClipboardContent::Image(data) => {
            hasher.update(b"image:");
            hasher.update(data);
        }
    }
    hasher.finalize().into()
}

/// Daemon for automatic clipboard synchronization
pub struct Daemon {
    config: DaemonConfig,
    key: Key,
    peers: Vec<SocketAddr>,
    bind_addr: SocketAddr,
    state: Arc<RwLock<DaemonState>>,
}

impl Daemon {
    /// Create a new daemon instance
    ///
    /// # Arguments
    ///
    /// * `config` - Daemon configuration
    /// * `key` - Encryption key (required for daemon mode)
    /// * `peers` - List of peer addresses to sync with
    /// * `bind_addr` - Address to bind listener
    pub fn new(
        config: DaemonConfig,
        key: Key,
        peers: Vec<SocketAddr>,
        bind_addr: SocketAddr,
    ) -> Self {
        Self {
            config,
            key,
            peers,
            bind_addr,
            state: Arc::new(RwLock::new(DaemonState::new())),
        }
    }

    /// Run the daemon
    ///
    /// This method runs until interrupted (Ctrl+C).
    pub async fn run(&self) -> anyhow::Result<()> {
        info!(
            "Starting cb-sync daemon on {} with {} peer(s)",
            self.bind_addr,
            self.peers.len()
        );

        for peer in &self.peers {
            info!("  Peer: {}", peer);
        }

        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        let cooldown = Duration::from_millis(self.config.sync_cooldown_ms);

        // Create clipboard instance
        let clipboard = create_clipboard();

        // Create listener
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Listening for incoming connections");

        // Main loop
        let mut poll_timer = interval(poll_interval);

        loop {
            tokio::select! {
                // Clipboard polling
                _ = poll_timer.tick() => {
                    if let Err(e) = self.poll_clipboard(clipboard.as_ref(), cooldown).await {
                        debug!("Clipboard poll error: {}", e);
                    }
                }

                // Incoming connections
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let state = Arc::clone(&self.state);
                            let key = self.key;
                            let clipboard = create_clipboard();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(
                                    stream, peer_addr, &key, clipboard.as_ref(), state
                                ).await {
                                    warn!("Connection from {} failed: {}", peer_addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Accept error: {}", e);
                        }
                    }
                }

                // Graceful shutdown
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Poll clipboard and broadcast changes
    async fn poll_clipboard(
        &self,
        clipboard: &dyn Clipboard,
        cooldown: Duration,
    ) -> anyhow::Result<()> {
        // Read current clipboard content
        let content = clipboard.read_content()?;
        let content_hash = hash_content(&content);

        // Check if content changed
        let mut state = self.state.write().await;

        if Some(content_hash) == state.last_content_hash {
            // No change
            return Ok(());
        }

        // Check loop prevention
        if Some(content_hash) == state.last_received_hash
            && state
                .last_received_time
                .is_some_and(|t| t.elapsed() < cooldown)
        {
            // This is content we just received, skip broadcasting
            debug!("Skipping broadcast of recently received content");
            state.last_content_hash = Some(content_hash);
            return Ok(());
        }

        // Content changed, broadcast to peers
        let content_type = match &content {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image(_) => "image",
        };
        info!(
            "Clipboard changed ({}), broadcasting to {} peer(s)",
            content_type,
            self.peers.len()
        );
        state.last_content_hash = Some(content_hash);
        drop(state); // Release lock before network operations

        self.broadcast_content(content).await;

        Ok(())
    }

    /// Broadcast content to all peers
    async fn broadcast_content(&self, content: ClipboardContent) {
        for peer in &self.peers {
            let peer = *peer;
            let key = self.key;
            let content = content.clone();
            let max_retries = self.config.max_retries;
            let retry_delay = Duration::from_millis(self.config.retry_delay_ms);

            tokio::spawn(async move {
                for attempt in 0..=max_retries {
                    match sync::send_content_encrypted(peer, content.clone(), &key).await {
                        Ok(()) => {
                            info!("Sent to {}", peer);
                            break;
                        }
                        Err(e) => {
                            if attempt < max_retries {
                                debug!(
                                    "Send to {} failed (attempt {}), retrying: {}",
                                    peer,
                                    attempt + 1,
                                    e
                                );
                                tokio::time::sleep(retry_delay).await;
                            } else {
                                warn!(
                                    "Failed to send to {} after {} attempts: {}",
                                    peer,
                                    max_retries + 1,
                                    e
                                );
                            }
                        }
                    }
                }
            });
        }
    }
}

/// Handle an incoming connection
async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    key: &Key,
    clipboard: &dyn Clipboard,
    state: Arc<RwLock<DaemonState>>,
) -> anyhow::Result<()> {
    debug!("Connection from {}", peer_addr);

    // Read message with timeout and size limit
    let limited = (&mut stream).take(MAX_MESSAGE_SIZE);
    let mut reader = BufReader::new(limited);
    let mut line = String::new();

    tokio::time::timeout(CLIPBOARD_TIMEOUT, reader.read_line(&mut line))
        .await
        .map_err(|_| anyhow::anyhow!("Read timeout"))??;

    if line.is_empty() {
        return Err(anyhow::anyhow!("Empty message"));
    }

    // Parse message
    let msg = Message::from_bytes(line.as_bytes())?;

    // Daemon mode requires encryption
    if !msg.is_encrypted() {
        warn!(
            "Rejecting unencrypted message from {} (daemon requires encryption)",
            peer_addr
        );
        return Err(anyhow::anyhow!(
            "Unencrypted messages not allowed in daemon mode"
        ));
    }

    // Decrypt content
    let content = msg.decrypt_content(key)?;
    let content_hash = hash_content(&content);

    // Send ack
    let ack = Message::ack().to_bytes()?;
    stream.write_all(&ack).await?;

    // Update state for loop prevention
    {
        let mut state = state.write().await;
        state.last_received_hash = Some(content_hash);
        state.last_received_time = Some(Instant::now());
        state.last_content_hash = Some(content_hash);
    }

    // Write to clipboard
    clipboard.write_content(&content)?;

    let content_type = match &content {
        ClipboardContent::Text(_) => "text",
        ClipboardContent::Image(_) => "image",
    };
    info!("Received {} from {}", content_type, peer_addr);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content_text() {
        let content1 = ClipboardContent::Text("hello".to_string());
        let content2 = ClipboardContent::Text("hello".to_string());
        let content3 = ClipboardContent::Text("world".to_string());

        assert_eq!(hash_content(&content1), hash_content(&content2));
        assert_ne!(hash_content(&content1), hash_content(&content3));
    }

    #[test]
    fn test_hash_content_image() {
        let content1 = ClipboardContent::Image(vec![1, 2, 3]);
        let content2 = ClipboardContent::Image(vec![1, 2, 3]);
        let content3 = ClipboardContent::Image(vec![4, 5, 6]);

        assert_eq!(hash_content(&content1), hash_content(&content2));
        assert_ne!(hash_content(&content1), hash_content(&content3));
    }

    #[test]
    fn test_hash_content_type_differs() {
        // Same bytes but different type should have different hash
        let text = ClipboardContent::Text("hello".to_string());
        let image = ClipboardContent::Image(b"hello".to_vec());

        assert_ne!(hash_content(&text), hash_content(&image));
    }

    #[tokio::test]
    async fn test_daemon_state_loop_prevention() {
        let state = Arc::new(RwLock::new(DaemonState::new()));
        let content = ClipboardContent::Text("test".to_string());
        let hash = hash_content(&content);

        // Simulate receiving content
        {
            let mut s = state.write().await;
            s.last_received_hash = Some(hash);
            s.last_received_time = Some(Instant::now());
        }

        // Check that loop prevention state is set
        {
            let s = state.read().await;
            assert_eq!(s.last_received_hash, Some(hash));
            assert!(s.last_received_time.is_some());
        }
    }
}
