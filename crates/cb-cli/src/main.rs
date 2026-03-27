//! cb-sync CLI application

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use cb_core::{
    ClipboardContent, ClipboardError, Config, create_clipboard, generate_key, key_from_base64,
    key_from_password, key_to_base64, sync,
};
use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "cb-sync")]
#[command(about = "Clipboard sync tool for sharing clipboard between devices")]
#[command(version)]
struct Cli {
    /// Enable verbose logging (-v for info, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Encryption password (or use CB_SYNC_PASSWORD env var)
    #[arg(short, long, global = true, env = "CB_SYNC_PASSWORD")]
    password: Option<String>,

    /// Encryption key in base64 (or use CB_SYNC_KEY env var)
    #[arg(short, long, global = true, env = "CB_SYNC_KEY")]
    key: Option<String>,

    /// Port to use (overrides config file)
    #[arg(short = 'P', long, global = true)]
    port: Option<u16>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Copy text to local clipboard
    Copy {
        /// Text to copy
        text: String,
    },

    /// Paste from local clipboard
    Paste,

    /// Send clipboard to a remote device
    Send {
        /// Remote host or named target (e.g., <IP_ADDRESS>, @home)
        host: String,

        /// Text to send (uses clipboard if not specified)
        text: Option<String>,

        /// Send image from clipboard (auto-detected if not specified)
        #[arg(short, long)]
        image: bool,

        /// Send image from file
        #[arg(short = 'f', long, conflicts_with = "text")]
        file: Option<PathBuf>,
    },

    /// Wait for one clipboard transfer from remote
    Receive {
        /// Address to bind (default: 0.0.0.0)
        #[arg(default_value = "0.0.0.0")]
        bind: String,

        /// Save received image to file (auto-generated if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Continuously receive clipboard data from remote
    Listen {
        /// Address to bind (default: 0.0.0.0)
        #[arg(default_value = "0.0.0.0")]
        bind: String,

        /// Directory to save received images
        #[arg(short, long)]
        image_dir: Option<PathBuf>,
    },

    /// Generate a new encryption key
    Keygen,

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Show detected display server and environment info
    Info,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Create a new config file with defaults
    Init {
        /// Overwrite existing config file
        #[arg(short, long)]
        force: bool,
    },

    /// Show current configuration
    Show,

    /// Show config file path
    Path,
}

fn init_logging(verbose: u8) {
    let level = match verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        _ => Level::DEBUG,
    };

    let filter = EnvFilter::from_default_env().add_directive(level.into());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}

/// Get encryption key from CLI args, env vars, or config file
/// Priority: CLI args > env vars > config file
fn get_key(cli: &Cli, config: &Config) -> Result<Option<[u8; 32]>> {
    // CLI args (already includes env var fallback via clap)
    if let Some(ref key_str) = cli.key {
        let key = key_from_base64(key_str).context("Invalid base64 key")?;
        return Ok(Some(key));
    }
    if let Some(ref password) = cli.password {
        return Ok(Some(key_from_password(password)));
    }

    // Config file
    if let Some(ref key_str) = config.encryption.key {
        let key = key_from_base64(key_str).context("Invalid base64 key in config")?;
        return Ok(Some(key));
    }
    if let Some(ref password) = config.encryption.password {
        return Ok(Some(key_from_password(password)));
    }

    Ok(None)
}

/// Get the port to use (CLI arg > config file > default)
fn get_port(cli: &Cli, config: &Config) -> u16 {
    cli.port.unwrap_or(config.general.port)
}

/// Get the verbosity level (CLI arg > config file)
fn get_verbose(cli: &Cli, config: &Config) -> u8 {
    if cli.verbose > 0 {
        cli.verbose
    } else {
        config.general.verbose
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load();

    let verbose = get_verbose(&cli, &config);
    init_logging(verbose);

    let key = get_key(&cli, &config)?;
    let port = get_port(&cli, &config);

    match cli.command {
        Some(Commands::Copy { text }) => {
            let clipboard = create_clipboard();
            clipboard.write_text(&text)?;
            eprintln!("Copied to clipboard");
        }

        Some(Commands::Paste) => {
            let clipboard = create_clipboard();
            match clipboard.read_text() {
                Ok(text) => print!("{}", text),
                Err(ClipboardError::Empty) => {
                    eprintln!("Clipboard is empty");
                }
                Err(e) => return Err(e.into()),
            }
        }

        Some(Commands::Send {
            host,
            text,
            image,
            file,
        }) => {
            let addr = resolve_target(&host, &config, port)?;

            // Determine content to send
            let content = if let Some(file_path) = file {
                // Read image from file
                let data = std::fs::read(&file_path)
                    .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
                ClipboardContent::Image(data)
            } else if let Some(t) = text {
                // Explicit text
                ClipboardContent::Text(t)
            } else {
                // Read from clipboard
                let clipboard = create_clipboard();
                if image || clipboard.has_image() {
                    // Try to read image
                    match clipboard.read_image() {
                        Ok(data) => ClipboardContent::Image(data),
                        Err(_) => {
                            // Fall back to text
                            let text = clipboard.read_text().context("Failed to read clipboard")?;
                            ClipboardContent::Text(text)
                        }
                    }
                } else {
                    let text = clipboard.read_text().context("Failed to read clipboard")?;
                    ClipboardContent::Text(text)
                }
            };

            let content_desc = match &content {
                ClipboardContent::Text(t) => format!("text ({} bytes)", t.len()),
                ClipboardContent::Image(d) => format!("image ({} bytes)", d.len()),
            };

            if let Some(ref k) = key {
                sync::send_content_encrypted(addr, content, k).await?;
                eprintln!("Sent {} to {} (encrypted)", content_desc, addr);
            } else {
                eprintln!("\x1b[33mWarning: Sending WITHOUT encryption!\x1b[0m");
                eprintln!("  Use: cb-sync keygen to generate an encryption key");
                sync::send_content(addr, content).await?;
                eprintln!("Sent {} to {} (unencrypted)", content_desc, addr);
            }
        }

        Some(Commands::Receive { bind, output }) => {
            let addr = parse_addr(&bind, port)?;

            if key.is_none() {
                eprintln!("\x1b[33mWarning: Receiving WITHOUT encryption!\x1b[0m");
                eprintln!("  Use: cb-sync keygen to generate an encryption key");
            }

            eprintln!(
                "Waiting for clipboard data on {}{}...",
                addr,
                if key.is_some() { " (encrypted)" } else { "" }
            );

            let content = if let Some(ref k) = key {
                sync::receive_once_content_encrypted(addr, k).await?
            } else {
                sync::receive_once_content(addr).await?
            };

            handle_received_content(content, output.as_ref())?;
        }

        Some(Commands::Listen { bind, image_dir }) => {
            let addr = parse_addr(&bind, port)?;

            if key.is_none() {
                eprintln!("\x1b[33mWarning: Listening WITHOUT encryption!\x1b[0m");
                eprintln!("  Use: cb-sync keygen to generate an encryption key");
            }

            eprintln!(
                "Listening on {}{}...",
                addr,
                if key.is_some() { " (encrypted)" } else { "" }
            );
            eprintln!("Press Ctrl+C to stop");

            let image_dir_clone = image_dir.clone();
            if let Some(k) = key {
                sync::listen_content_encrypted(addr, &k, move |content| {
                    if let Err(e) = handle_received_content(content, image_dir_clone.as_ref()) {
                        eprintln!("Error handling content: {}", e);
                    }
                })
                .await?;
            } else {
                sync::listen_content(addr, move |content| {
                    if let Err(e) = handle_received_content(content, image_dir.as_ref()) {
                        eprintln!("Error handling content: {}", e);
                    }
                })
                .await?;
            }
        }

        Some(Commands::Keygen) => {
            let key = generate_key();
            let encoded = key_to_base64(&key);
            println!("{}", encoded);
            eprintln!();
            eprintln!("Add to config file (~/.config/cb-sync/config.toml):");
            eprintln!("  [encryption]");
            eprintln!("  key = \"<paste key above>\"");
            eprintln!();
            eprintln!("Or use directly (note: visible in process list):");
            eprintln!("  cb-sync -k '<KEY>' send/receive/listen");
        }

        Some(Commands::Config { action }) => match action {
            ConfigAction::Init { force } => {
                let path = Config::default_path().context("Could not determine config path")?;

                if path.exists() && !force {
                    bail!(
                        "Config file already exists at {}\nUse --force to overwrite",
                        path.display()
                    );
                }

                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, Config::example())?;
                eprintln!("Created config file: {}", path.display());
            }

            ConfigAction::Show => {
                let path = Config::default_path();
                if let Some(ref p) = path {
                    if p.exists() {
                        eprintln!("# Config file: {}", p.display());
                        eprintln!();
                    } else {
                        eprintln!("# No config file (using defaults)");
                        eprintln!("# Run `cb-sync config init` to create one");
                        eprintln!();
                    }
                }

                println!("[general]");
                println!("port = {}", config.general.port);
                println!("timeout_secs = {}", config.general.timeout_secs);
                println!("verbose = {}", config.general.verbose);
                println!();
                println!("[encryption]");
                if let Some(ref pw) = config.encryption.password {
                    println!("password = \"{}\"", "*".repeat(pw.len()));
                } else {
                    println!("# password = \"...\"");
                }
                if let Some(ref k) = config.encryption.key {
                    println!("key = \"{}...\"", &k[..8.min(k.len())]);
                } else {
                    println!("# key = \"...\"");
                }
                println!();
                println!("[targets]");
                if let Some(ref default) = config.targets.default {
                    println!("default = \"{}\"", default);
                } else {
                    println!("# default = \"<TARGET_IP>\"");
                }
                for (name, addr) in &config.targets.named {
                    println!("{} = \"{}\"", name, addr);
                }
            }

            ConfigAction::Path => match Config::default_path() {
                Some(path) => {
                    println!("{}", path.display());
                    if path.exists() {
                        eprintln!("(exists)");
                    } else {
                        eprintln!("(not created yet)");
                    }
                }
                None => {
                    bail!("Could not determine config directory");
                }
            },
        },

        Some(Commands::Info) => {
            let server = cb_core::detect_display_server();
            println!("Display server: {:?}", server);
            println!("Default port: {}", port);
            println!(
                "Encryption: {}",
                if key.is_some() { "enabled" } else { "disabled" }
            );

            if let Some(path) = Config::default_path() {
                println!(
                    "Config file: {} ({})",
                    path.display(),
                    if path.exists() {
                        "exists"
                    } else {
                        "not created"
                    }
                );
            }

            let clipboard = create_clipboard();

            // Check for image
            if clipboard.has_image() {
                match clipboard.read_image() {
                    Ok(data) => println!("Clipboard: image ({} bytes)", data.len()),
                    Err(_) => println!("Clipboard: image (failed to read)"),
                }
            } else {
                match clipboard.read_text() {
                    Ok(text) => println!("Clipboard: text ({} bytes)", text.len()),
                    Err(ClipboardError::Empty) => println!("Clipboard: empty"),
                    Err(e) => println!("Clipboard: error ({})", e),
                }
            }
        }

        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}

/// Handle received clipboard content
fn handle_received_content(content: ClipboardContent, output_path: Option<&PathBuf>) -> Result<()> {
    let clipboard = create_clipboard();

    match content {
        ClipboardContent::Text(text) => {
            clipboard.write_text(&text)?;
            eprintln!(
                "Received text and copied to clipboard ({} bytes)",
                text.len()
            );
        }
        ClipboardContent::Image(data) => {
            let size = data.len();

            // Try to write to clipboard
            let clipboard_result = clipboard.write_image(&data);

            // Determine output path
            let path = if let Some(p) = output_path {
                if p.is_dir() {
                    // Generate filename in directory
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    p.join(format!("cb-sync-{}.png", timestamp))
                } else {
                    p.clone()
                }
            } else {
                // Generate in current directory
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                PathBuf::from(format!("cb-sync-{}.png", timestamp))
            };

            // Save to file
            std::fs::write(&path, &data)?;

            if clipboard_result.is_ok() {
                eprintln!(
                    "Received image ({} bytes), copied to clipboard and saved to {}",
                    size,
                    path.display()
                );
            } else {
                eprintln!(
                    "Received image ({} bytes), saved to {} (clipboard write failed)",
                    size,
                    path.display()
                );
            }
        }
    }

    Ok(())
}

/// Resolve a target string to a socket address
/// Supports: IP address, hostname, or named target (prefixed with @)
fn resolve_target(target: &str, config: &Config, default_port: u16) -> Result<SocketAddr> {
    let host = if let Some(name) = target.strip_prefix('@') {
        // Named target from config
        config
            .targets
            .named
            .get(name)
            .cloned()
            .with_context(|| format!("Unknown target '{}'. Check your config file.", name))?
    } else if target == "default" || target.is_empty() {
        // Default target
        config
            .targets
            .default
            .clone()
            .context("No default target configured. Use `cb-sync config init` to set one.")?
    } else {
        target.to_string()
    };

    parse_addr(&host, default_port)
}

fn parse_addr(s: &str, default_port: u16) -> Result<SocketAddr> {
    if s.contains(':') {
        s.parse().context("Invalid address format")
    } else {
        format!("{}:{}", s, default_port)
            .parse()
            .context("Invalid address format")
    }
}
