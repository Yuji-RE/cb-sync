//! Clipboard sync over TCP
//!
//! Provides sender and receiver for clipboard synchronization.
//! Supports both plaintext and encrypted modes, for text and images.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, instrument, warn};

use crate::clipboard::ClipboardContent;
use crate::crypto::Key;
use crate::error::SyncError;
use crate::protocol::Message;

/// Default port for clipboard sync
pub const DEFAULT_PORT: u16 = 34812;

/// Timeout for receiving clipboard data (20 seconds as per spec)
pub const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(20);

/// Maximum message size to prevent DoS attacks (100 MiB for large images)
pub const MAX_MESSAGE_SIZE: u64 = 100 * 1024 * 1024;

/// Result type for sync operations
pub type Result<T> = std::result::Result<T, SyncError>;

/// Send clipboard text to a remote peer (plaintext)
#[instrument(skip(text), fields(text_len = text.len()))]
pub async fn send(addr: SocketAddr, text: String) -> Result<()> {
    send_internal(addr, Message::clipboard(text)).await
}

/// Send clipboard text to a remote peer (encrypted)
#[instrument(skip(text, key), fields(text_len = text.len(), encrypted = true))]
pub async fn send_encrypted(addr: SocketAddr, text: String, key: &Key) -> Result<()> {
    let msg = Message::encrypted_text(&text, key)?;
    send_internal(addr, msg).await
}

/// Send clipboard content (text or image) to a remote peer (plaintext)
#[instrument(skip(content), fields(content_type = ?content_type(&content)))]
pub async fn send_content(addr: SocketAddr, content: ClipboardContent) -> Result<()> {
    send_internal(addr, Message::from_content(content)).await
}

/// Send clipboard content (text or image) to a remote peer (encrypted)
#[instrument(skip(content, key), fields(content_type = ?content_type(&content), encrypted = true))]
pub async fn send_content_encrypted(
    addr: SocketAddr,
    content: ClipboardContent,
    key: &Key,
) -> Result<()> {
    let msg = Message::encrypted_content(&content, key)?;
    send_internal(addr, msg).await
}

fn content_type(content: &ClipboardContent) -> &'static str {
    match content {
        ClipboardContent::Text(_) => "text",
        ClipboardContent::Image(_) => "image",
    }
}

async fn send_internal(addr: SocketAddr, msg: Message) -> Result<()> {
    info!("Connecting to {}", addr);
    let mut stream = TcpStream::connect(addr).await?;
    debug!("Connected");

    let bytes = msg.to_bytes()?;
    stream.write_all(&bytes).await?;
    debug!(
        "Sent clipboard data (encrypted: {}, type: {:?})",
        msg.is_encrypted(),
        msg.content_type()
    );

    // Wait for ack (limit read size to prevent DoS)
    let limited = (&mut stream).take(1024); // ACK is small
    let mut reader = BufReader::new(limited);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let response = Message::from_bytes(line.as_bytes())?;
    match response {
        Message::Ack => {
            info!("Received ack");
            Ok(())
        }
        _ => {
            warn!("Unexpected response");
            Err(SyncError::UnexpectedMessage)
        }
    }
}

/// Callback for handling received clipboard data
pub type OnReceive = Box<dyn Fn(String) + Send + Sync>;

/// Callback for handling received clipboard content (text or image)
pub type OnReceiveContent = Box<dyn Fn(ClipboardContent) + Send + Sync>;

/// Start listening for incoming clipboard data (plaintext only)
///
/// Returns the received text when a message arrives, or times out.
#[instrument]
pub async fn receive_once(addr: SocketAddr) -> Result<String> {
    let content = receive_once_content_internal(addr, None).await?;
    match content {
        ClipboardContent::Text(text) => Ok(text),
        ClipboardContent::Image(_) => Err(SyncError::UnexpectedMessage),
    }
}

/// Start listening for incoming encrypted clipboard data
#[instrument(skip(key))]
pub async fn receive_once_encrypted(addr: SocketAddr, key: &Key) -> Result<String> {
    let content = receive_once_content_internal(addr, Some(key)).await?;
    match content {
        ClipboardContent::Text(text) => Ok(text),
        ClipboardContent::Image(_) => Err(SyncError::UnexpectedMessage),
    }
}

/// Start listening for incoming clipboard content (text or image)
#[instrument]
pub async fn receive_once_content(addr: SocketAddr) -> Result<ClipboardContent> {
    receive_once_content_internal(addr, None).await
}

/// Start listening for incoming encrypted clipboard content (text or image)
#[instrument(skip(key))]
pub async fn receive_once_content_encrypted(
    addr: SocketAddr,
    key: &Key,
) -> Result<ClipboardContent> {
    receive_once_content_internal(addr, Some(key)).await
}

async fn receive_once_content_internal(
    addr: SocketAddr,
    key: Option<&Key>,
) -> Result<ClipboardContent> {
    info!("Binding to {}", addr);
    let listener = TcpListener::bind(addr).await?;

    info!("Waiting for connection (timeout: {:?})", CLIPBOARD_TIMEOUT);
    let (mut stream, peer) = tokio::time::timeout(CLIPBOARD_TIMEOUT, listener.accept())
        .await
        .map_err(|_| SyncError::Timeout)??;

    info!("Connection from {}", peer);

    // Limit message size to prevent DoS attacks
    let limited = (&mut stream).take(MAX_MESSAGE_SIZE);
    let mut reader = BufReader::new(limited);
    let mut line = String::new();

    tokio::time::timeout(CLIPBOARD_TIMEOUT, reader.read_line(&mut line))
        .await
        .map_err(|_| SyncError::Timeout)??;

    if line.is_empty() {
        return Err(SyncError::UnexpectedMessage);
    }

    let msg = Message::from_bytes(line.as_bytes())?;
    let content = extract_content(&msg, key)?;

    debug!("Received {:?}", content_type(&content));
    // Send ack
    let ack = Message::ack().to_bytes()?;
    stream.write_all(&ack).await?;
    info!("Sent ack");
    Ok(content)
}

/// Start a receiver that continuously listens for clipboard data (plaintext only)
#[instrument(skip(on_receive))]
pub async fn listen<F>(addr: SocketAddr, on_receive: F) -> Result<()>
where
    F: Fn(String) + Send + Sync,
{
    listen_content(addr, move |content| {
        if let ClipboardContent::Text(text) = content {
            on_receive(text);
        }
    })
    .await
}

/// Start a receiver that continuously listens for encrypted clipboard data
#[instrument(skip(key, on_receive))]
pub async fn listen_encrypted<F>(addr: SocketAddr, key: &Key, on_receive: F) -> Result<()>
where
    F: Fn(String) + Send + Sync,
{
    listen_content_encrypted(addr, key, move |content| {
        if let ClipboardContent::Text(text) = content {
            on_receive(text);
        }
    })
    .await
}

/// Start a receiver that continuously listens for clipboard content (text or image)
#[instrument(skip(on_receive))]
pub async fn listen_content<F>(addr: SocketAddr, on_receive: F) -> Result<()>
where
    F: Fn(ClipboardContent) + Send + Sync,
{
    listen_content_internal(addr, None, on_receive).await
}

/// Start a receiver that continuously listens for encrypted clipboard content
#[instrument(skip(key, on_receive))]
pub async fn listen_content_encrypted<F>(addr: SocketAddr, key: &Key, on_receive: F) -> Result<()>
where
    F: Fn(ClipboardContent) + Send + Sync,
{
    listen_content_internal(addr, Some(key), on_receive).await
}

async fn listen_content_internal<F>(
    addr: SocketAddr,
    key: Option<&Key>,
    on_receive: F,
) -> Result<()>
where
    F: Fn(ClipboardContent) + Send + Sync,
{
    info!("Binding to {}", addr);
    let listener = TcpListener::bind(addr).await?;
    info!("Listening for connections (encrypted: {})", key.is_some());

    loop {
        let accept_result = tokio::time::timeout(CLIPBOARD_TIMEOUT, listener.accept()).await;

        match accept_result {
            Ok(Ok((mut stream, peer))) => {
                debug!("Connection from {}", peer);
                // Limit message size to prevent DoS attacks
                let limited = (&mut stream).take(MAX_MESSAGE_SIZE);
                let mut reader = BufReader::new(limited);
                let mut line = String::new();

                // Apply timeout to read to prevent DoS from stalling clients
                let read_result =
                    tokio::time::timeout(CLIPBOARD_TIMEOUT, reader.read_line(&mut line)).await;

                match read_result {
                    Ok(Ok(_)) if !line.is_empty() => {
                        if let Ok(msg) = Message::from_bytes(line.as_bytes()) {
                            if let Ok(content) = extract_content(&msg, key) {
                                info!("Received {:?} from {}", content_type(&content), peer);
                                // Send ack
                                if let Ok(ack) = Message::ack().to_bytes() {
                                    let _ = stream.write_all(&ack).await;
                                }
                                on_receive(content);
                            } else {
                                warn!("Failed to decrypt message from {}", peer);
                            }
                        }
                    }
                    Err(_) => {
                        debug!("Read timeout from {}, dropping connection", peer);
                    }
                    _ => {}
                }
            }
            Ok(Err(e)) => {
                warn!("Accept error: {}", e);
                continue;
            }
            Err(_) => {
                debug!("No connection in {:?}, continuing", CLIPBOARD_TIMEOUT);
                continue;
            }
        }
    }
}

fn extract_content(msg: &Message, key: Option<&Key>) -> Result<ClipboardContent> {
    match (msg, key) {
        (Message::Clipboard { text, .. }, _) => Ok(ClipboardContent::Text(text.clone())),
        (Message::Image { .. }, _) => msg.content().map_err(|e| e.into()),
        (Message::Encrypted { .. }, Some(k)) => msg.decrypt_content(k).map_err(|e| e.into()),
        (Message::Encrypted { .. }, None) => {
            warn!("Received encrypted message but no key provided");
            Err(SyncError::UnexpectedMessage)
        }
        _ => Err(SyncError::UnexpectedMessage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_key;

    #[tokio::test]
    async fn send_receive_roundtrip() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let msg = Message::from_bytes(line.as_bytes()).unwrap();
            let text = match msg {
                Message::Clipboard { text, .. } => text,
                _ => panic!("expected clipboard message"),
            };

            let ack = Message::ack().to_bytes().unwrap();
            stream.write_all(&ack).await.unwrap();

            text
        });

        send(bound_addr, "test message".to_string()).await.unwrap();

        let received = receiver.await.unwrap();
        assert_eq!(received, "test message");
    }

    #[tokio::test]
    async fn encrypted_send_receive_roundtrip() {
        let key = generate_key();
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let key_clone = key;
        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let msg = Message::from_bytes(line.as_bytes()).unwrap();
            assert!(msg.is_encrypted());

            let text = msg.decrypt(&key_clone).unwrap();

            let ack = Message::ack().to_bytes().unwrap();
            stream.write_all(&ack).await.unwrap();

            text
        });

        send_encrypted(bound_addr, "secret message".to_string(), &key)
            .await
            .unwrap();

        let received = receiver.await.unwrap();
        assert_eq!(received, "secret message");
    }

    #[tokio::test]
    async fn image_send_receive_roundtrip() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        // Minimal PNG (1x1 transparent pixel)
        let png_data: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49,
            0x44, 0x41, 0x54, // IDAT chunk
            0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x49, 0x45, 0x4E, 0x44, // IEND chunk
            0xAE, 0x42, 0x60, 0x82,
        ];

        let png_clone = png_data.clone();
        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let msg = Message::from_bytes(line.as_bytes()).unwrap();
            assert!(msg.is_image());

            let content = msg.content().unwrap();

            let ack = Message::ack().to_bytes().unwrap();
            stream.write_all(&ack).await.unwrap();

            content
        });

        send_content(bound_addr, ClipboardContent::Image(png_data.clone()))
            .await
            .unwrap();

        let received = receiver.await.unwrap();
        assert_eq!(received, ClipboardContent::Image(png_clone));
    }
}
