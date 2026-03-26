//! Configuration file support for cb-sync
//!
//! Configuration is loaded from `~/.config/cb-sync/config.toml` on Linux/macOS
//! or `%APPDATA%\cb-sync\config.toml` on Windows.
//!
//! # Example config.toml
//!
//! ```toml
//! [general]
//! port = 34812
//! timeout_secs = 20
//! verbose = 0
//!
//! [encryption]
//! password = "shared-secret"
//! # Or use a key instead:
//! # key = "base64-encoded-key"
//!
//! [targets]
//! default = "<TARGET_IP>"
//! home = "<HOME_IP>"
//! work = "<WORK_IP>"
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::sync::DEFAULT_PORT;

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub encryption: EncryptionConfig,
    pub targets: TargetConfig,
}

/// General settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Default port for connections
    pub port: u16,
    /// Timeout in seconds (default: 20)
    pub timeout_secs: u64,
    /// Verbosity level (0=warn, 1=info, 2=debug)
    pub verbose: u8,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            timeout_secs: 20,
            verbose: 0,
        }
    }
}

/// Encryption settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EncryptionConfig {
    /// Encryption password (derived to key using SHA256)
    pub password: Option<String>,
    /// Encryption key in base64 format
    pub key: Option<String>,
}

impl EncryptionConfig {
    /// Check if encryption is configured
    pub fn is_enabled(&self) -> bool {
        self.password.is_some() || self.key.is_some()
    }
}

/// Target host configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetConfig {
    /// Default target host
    pub default: Option<String>,
    /// Named targets (e.g., "home" -> "<TARGET_IP>")
    #[serde(flatten)]
    pub named: HashMap<String, String>,
}

impl TargetConfig {
    /// Resolve a target name to an address
    /// If the input is already an IP/hostname, returns it as-is
    /// If it's a named target, looks it up in the config
    pub fn resolve(&self, name_or_addr: &str) -> Option<String> {
        // Check if it looks like an address (contains dots or colons)
        if name_or_addr.contains('.') || name_or_addr.contains(':') {
            return Some(name_or_addr.to_string());
        }

        // Try to find it as a named target
        self.named.get(name_or_addr).cloned()
    }
}

impl Config {
    /// Get the default config file path
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("cb-sync").join("config.toml"))
    }

    /// Load config from the default path
    /// Returns default config if file doesn't exist
    pub fn load() -> Self {
        Self::default_path()
            .and_then(|path| Self::load_from(&path).ok())
            .unwrap_or_default()
    }

    /// Load config from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(ConfigError::Io)?;
        toml::from_str(&content).map_err(ConfigError::Parse)
    }

    /// Save config to the default path
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::default_path().ok_or(ConfigError::NoConfigDir)?;
        self.save_to(&path)
    }

    /// Save config to a specific path
    pub fn save_to(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }

        let content = toml::to_string_pretty(self).map_err(ConfigError::Serialize)?;
        fs::write(path, content).map_err(ConfigError::Io)?;
        Ok(())
    }

    /// Generate an example config file content
    pub fn example() -> &'static str {
        r#"# cb-sync configuration file
# Location: ~/.config/cb-sync/config.toml

[general]
# Default port for connections
port = 34812

# Timeout in seconds for clipboard sync
timeout_secs = 20

# Verbosity level: 0=warn, 1=info, 2=debug
verbose = 0

[encryption]
# Use either password or key, not both
# Password is hashed to derive the encryption key
# password = "your-shared-secret"

# Or use a pre-generated key (from `cb-sync keygen`)
# key = "base64-encoded-key"

[targets]
# Default target host for `cb-sync send`
# default = "<TARGET_IP>"

# Named targets for easy reference
# Example: `cb-sync send @home` uses <TARGET_IP>
# home = "<TARGET_IP>"
# work = "<WORK_IP>"
"#
    }
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("Could not determine config directory")]
    NoConfigDir,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.port, DEFAULT_PORT);
        assert_eq!(config.general.timeout_secs, 20);
        assert!(!config.encryption.is_enabled());
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
[general]
port = 12345
timeout_secs = 30
verbose = 1

[encryption]
password = "test-password"

[targets]
default = "<EXAMPLE_IP>"
office = "10.0.0.1"
"#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.general.port, 12345);
        assert_eq!(config.general.timeout_secs, 30);
        assert_eq!(config.general.verbose, 1);
        assert_eq!(
            config.encryption.password,
            Some("test-password".to_string())
        );
        assert!(config.encryption.is_enabled());
        assert_eq!(config.targets.default, Some("<EXAMPLE_IP>".to_string()));
        assert_eq!(
            config.targets.named.get("office"),
            Some(&"10.0.0.1".to_string())
        );
    }

    #[test]
    fn test_target_resolve() {
        let mut targets = TargetConfig::default();
        targets
            .named
            .insert("home".to_string(), "<TARGET_IP>".to_string());

        // Direct address
        assert_eq!(
            targets.resolve("<HOME_IP>"),
            Some("<HOME_IP>".to_string())
        );

        // Named target
        assert_eq!(targets.resolve("home"), Some("<TARGET_IP>".to_string()));

        // Unknown name
        assert_eq!(targets.resolve("unknown"), None);
    }

    #[test]
    fn test_example_config_parses() {
        let example = Config::example();
        // Should parse without errors (comments are ignored)
        let _: Config = toml::from_str(example).unwrap();
    }
}
