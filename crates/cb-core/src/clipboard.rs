//! Clipboard abstraction layer
//!
//! Provides a platform-agnostic interface for clipboard operations.
//! Supports Wayland (wl-copy/wl-paste), X11 (xclip), and Windows.

use std::process::Command;

use crate::error::ClipboardError;

/// Result type for clipboard operations
pub type Result<T> = std::result::Result<T, ClipboardError>;

/// Clipboard content type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    /// Text content
    Text(String),
    /// Image content (PNG format)
    Image(Vec<u8>),
}

impl ClipboardContent {
    /// Check if this is text content
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Check if this is image content
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Image(_))
    }

    /// Get text content if available
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Get image content if available
    pub fn as_image(&self) -> Option<&[u8]> {
        match self {
            Self::Image(data) => Some(data),
            _ => None,
        }
    }
}

/// Platform-agnostic clipboard interface
pub trait Clipboard: Send + Sync {
    /// Read text from the clipboard
    fn read_text(&self) -> Result<String>;

    /// Write text to the clipboard
    fn write_text(&self, text: &str) -> Result<()>;

    /// Read image from the clipboard (PNG format)
    fn read_image(&self) -> Result<Vec<u8>>;

    /// Write image to the clipboard (PNG format)
    fn write_image(&self, data: &[u8]) -> Result<()>;

    /// Check if clipboard contains an image
    fn has_image(&self) -> bool;

    /// Read content (auto-detect text or image)
    fn read_content(&self) -> Result<ClipboardContent> {
        if self.has_image() {
            self.read_image().map(ClipboardContent::Image)
        } else {
            self.read_text().map(ClipboardContent::Text)
        }
    }

    /// Write content to clipboard
    fn write_content(&self, content: &ClipboardContent) -> Result<()> {
        match content {
            ClipboardContent::Text(text) => self.write_text(text),
            ClipboardContent::Image(data) => self.write_image(data),
        }
    }
}

/// Wayland clipboard implementation using wl-copy/wl-paste
#[derive(Debug, Default)]
pub struct WaylandClipboard;

impl WaylandClipboard {
    pub fn new() -> Self {
        Self
    }

    fn check_command(cmd: &str) -> Result<()> {
        match Command::new("which").arg(cmd).output() {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(ClipboardError::CommandNotFound(cmd.to_string())),
        }
    }

    /// Check if Wayland clipboard tools are available
    pub fn is_available() -> bool {
        Self::check_command("wl-paste").is_ok() && Self::check_command("wl-copy").is_ok()
    }
}

impl Clipboard for WaylandClipboard {
    fn read_text(&self) -> Result<String> {
        Self::check_command("wl-paste")?;

        let output = Command::new("wl-paste").arg("--no-newline").output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No selection") {
                return Err(ClipboardError::Empty);
            }
            return Err(ClipboardError::CommandFailed(stderr.into_owned()));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn write_text(&self, text: &str) -> Result<()> {
        Self::check_command("wl-copy")?;

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::CommandFailed(
                "wl-copy exited with non-zero status".to_string(),
            ));
        }

        Ok(())
    }

    fn read_image(&self) -> Result<Vec<u8>> {
        Self::check_command("wl-paste")?;

        let output = Command::new("wl-paste")
            .args(["--type", "image/png"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No selection") || stderr.contains("No suitable type") {
                return Err(ClipboardError::Empty);
            }
            return Err(ClipboardError::CommandFailed(stderr.into_owned()));
        }

        if output.stdout.is_empty() {
            return Err(ClipboardError::Empty);
        }

        Ok(output.stdout)
    }

    fn write_image(&self, data: &[u8]) -> Result<()> {
        Self::check_command("wl-copy")?;

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("wl-copy")
            .args(["--type", "image/png"])
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(data)?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::CommandFailed(
                "wl-copy exited with non-zero status".to_string(),
            ));
        }

        Ok(())
    }

    fn has_image(&self) -> bool {
        // Check available MIME types
        if let Ok(output) = Command::new("wl-paste").arg("--list-types").output() {
            let types = String::from_utf8_lossy(&output.stdout);
            return types.contains("image/png") || types.contains("image/");
        }
        false
    }
}

/// X11 clipboard implementation using xclip
#[derive(Debug, Default)]
pub struct X11Clipboard;

impl X11Clipboard {
    pub fn new() -> Self {
        Self
    }

    fn check_command(cmd: &str) -> Result<()> {
        match Command::new("which").arg(cmd).output() {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(ClipboardError::CommandNotFound(cmd.to_string())),
        }
    }

    /// Check if X11 clipboard tools are available
    pub fn is_available() -> bool {
        Self::check_command("xclip").is_ok()
    }
}

impl Clipboard for X11Clipboard {
    fn read_text(&self) -> Result<String> {
        Self::check_command("xclip")?;

        let output = Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("There is no owner") {
                return Err(ClipboardError::Empty);
            }
            return Err(ClipboardError::CommandFailed(stderr.into_owned()));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn write_text(&self, text: &str) -> Result<()> {
        Self::check_command("xclip")?;

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::CommandFailed(
                "xclip exited with non-zero status".to_string(),
            ));
        }

        Ok(())
    }

    fn read_image(&self) -> Result<Vec<u8>> {
        Self::check_command("xclip")?;

        let output = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "image/png", "-o"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("There is no owner") || stderr.contains("target image/png") {
                return Err(ClipboardError::Empty);
            }
            return Err(ClipboardError::CommandFailed(stderr.into_owned()));
        }

        if output.stdout.is_empty() {
            return Err(ClipboardError::Empty);
        }

        Ok(output.stdout)
    }

    fn write_image(&self, data: &[u8]) -> Result<()> {
        Self::check_command("xclip")?;

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "image/png"])
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(data)?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::CommandFailed(
                "xclip exited with non-zero status".to_string(),
            ));
        }

        Ok(())
    }

    fn has_image(&self) -> bool {
        // Try to read image to check if available
        if let Ok(output) = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
            .output()
        {
            let targets = String::from_utf8_lossy(&output.stdout);
            return targets.contains("image/png") || targets.contains("image/");
        }
        false
    }
}

/// WSL clipboard implementation using clip.exe and powershell.exe
#[derive(Debug, Default)]
pub struct WslClipboard;

impl WslClipboard {
    pub fn new() -> Self {
        Self
    }

    /// Check if WSL clipboard tools are available
    pub fn is_available() -> bool {
        // Check if clip.exe exists (should be in /mnt/c/Windows/System32/)
        Command::new("clip.exe")
            .arg("/?")
            .output()
            .map(|o| o.status.success() || !o.stderr.is_empty()) // clip.exe returns help on /?
            .unwrap_or(false)
    }
}

impl Clipboard for WslClipboard {
    fn read_text(&self) -> Result<String> {
        // Use PowerShell to read clipboard
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Get-Clipboard"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ClipboardError::CommandFailed(format!(
                "powershell Get-Clipboard failed: {}",
                stderr
            )));
        }

        let text = String::from_utf8(output.stdout)?;
        // PowerShell adds a trailing newline, remove it
        let text = text.trim_end_matches(['\r', '\n']).to_string();

        if text.is_empty() {
            return Err(ClipboardError::Empty);
        }

        Ok(text)
    }

    fn write_text(&self, text: &str) -> Result<()> {
        use std::io::Write;
        use std::process::Stdio;

        // Use clip.exe to write to clipboard
        let mut child = Command::new("clip.exe").stdin(Stdio::piped()).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::CommandFailed(
                "clip.exe exited with non-zero status".to_string(),
            ));
        }

        Ok(())
    }

    fn read_image(&self) -> Result<Vec<u8>> {
        // TODO: Implement WSL image clipboard using PowerShell
        // PowerShell can read images but conversion to PNG is complex
        Err(ClipboardError::CommandFailed(
            "WSL image clipboard not yet implemented".to_string(),
        ))
    }

    fn write_image(&self, _data: &[u8]) -> Result<()> {
        // TODO: Implement WSL image clipboard
        Err(ClipboardError::CommandFailed(
            "WSL image clipboard not yet implemented".to_string(),
        ))
    }

    fn has_image(&self) -> bool {
        // TODO: Check if clipboard contains image via PowerShell
        false
    }
}

/// Windows clipboard implementation using clipboard-win crate
#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
pub struct WindowsClipboard;

#[cfg(target_os = "windows")]
impl WindowsClipboard {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available() -> bool {
        true
    }
}

#[cfg(target_os = "windows")]
impl Clipboard for WindowsClipboard {
    fn read_text(&self) -> Result<String> {
        use clipboard_win::{formats, get_clipboard};

        match get_clipboard::<String, _>(formats::Unicode) {
            Ok(text) => {
                if text.is_empty() {
                    Err(ClipboardError::Empty)
                } else {
                    Ok(text)
                }
            }
            Err(e) => {
                // Error code 0 typically means empty clipboard
                if e.raw_code() == 0 {
                    Err(ClipboardError::Empty)
                } else {
                    Err(ClipboardError::CommandFailed(format!(
                        "Failed to read clipboard: {}",
                        e
                    )))
                }
            }
        }
    }

    fn write_text(&self, text: &str) -> Result<()> {
        use clipboard_win::{formats, set_clipboard};

        set_clipboard(formats::Unicode, text)
            .map_err(|e| ClipboardError::CommandFailed(format!("Failed to write clipboard: {}", e)))
    }

    fn read_image(&self) -> Result<Vec<u8>> {
        // TODO: Implement Windows image clipboard using DIB format + PNG conversion
        // This requires the `image` crate for bitmap to PNG conversion
        Err(ClipboardError::CommandFailed(
            "Windows image clipboard not yet implemented".to_string(),
        ))
    }

    fn write_image(&self, _data: &[u8]) -> Result<()> {
        // TODO: Implement Windows image clipboard using DIB format
        Err(ClipboardError::CommandFailed(
            "Windows image clipboard not yet implemented".to_string(),
        ))
    }

    fn has_image(&self) -> bool {
        // TODO: Check for CF_DIB or CF_BITMAP format
        false
    }
}

/// Detected display server type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Windows,
    Wsl,
    Unknown,
}

/// Check if running inside WSL (Windows Subsystem for Linux)
fn is_wsl() -> bool {
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        let version_lower = version.to_lowercase();
        return version_lower.contains("microsoft") || version_lower.contains("wsl");
    }
    false
}

/// Detect the current display server
pub fn detect_display_server() -> DisplayServer {
    #[cfg(target_os = "windows")]
    {
        return DisplayServer::Windows;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Check for WSL first (before Wayland/X11 since those env vars may be set but tools don't work)
        if is_wsl() {
            return DisplayServer::Wsl;
        }

        // Check for Wayland
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServer::Wayland;
        }

        // Check for X11
        if std::env::var("DISPLAY").is_ok() {
            return DisplayServer::X11;
        }

        DisplayServer::Unknown
    }
}

/// Create a clipboard instance for the current platform
pub fn create_clipboard() -> Box<dyn Clipboard> {
    match detect_display_server() {
        DisplayServer::Wayland => Box::new(WaylandClipboard::new()),
        DisplayServer::X11 => Box::new(X11Clipboard::new()),
        DisplayServer::Wsl => Box::new(WslClipboard::new()),
        #[cfg(target_os = "windows")]
        DisplayServer::Windows => Box::new(WindowsClipboard::new()),
        #[cfg(not(target_os = "windows"))]
        DisplayServer::Windows => Box::new(WaylandClipboard::new()), // fallback
        DisplayServer::Unknown => {
            // Try WSL first, then Wayland, then X11
            if is_wsl() {
                Box::new(WslClipboard::new())
            } else if WaylandClipboard::is_available() {
                Box::new(WaylandClipboard::new())
            } else {
                Box::new(X11Clipboard::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_clipboard_can_be_created() {
        let _clipboard = WaylandClipboard::new();
    }

    #[test]
    fn x11_clipboard_can_be_created() {
        let _clipboard = X11Clipboard::new();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_clipboard_can_be_created() {
        let _clipboard = WindowsClipboard::new();
        assert!(WindowsClipboard::is_available());
    }

    #[test]
    fn wsl_clipboard_can_be_created() {
        let _clipboard = WslClipboard::new();
    }

    #[test]
    fn create_clipboard_returns_boxed_trait() {
        let _clipboard = create_clipboard();
    }

    #[test]
    fn detect_display_server_returns_value() {
        let server = detect_display_server();
        // Just verify it doesn't panic
        let _ = format!("{:?}", server);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_clipboard_roundtrip() {
        let clipboard = WindowsClipboard::new();
        let test_text = "cb-sync test: クリップボードテスト 🎉";

        // Write to clipboard
        clipboard
            .write_text(test_text)
            .expect("write should succeed");

        // Read back
        let result = clipboard.read_text().expect("read should succeed");
        assert_eq!(result, test_text);
    }
}
