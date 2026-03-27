# cb-sync

Cross-platform clipboard synchronization tool written in Rust.

## Overview

Securely share clipboard contents between Windows/WSL/Linux machines over LAN. Unlike always-on clipboard managers, cb-sync only syncs when explicitly triggered, giving you full control over what gets shared.

**Key Features:**
- Text and image clipboard sync
- End-to-end encryption (ChaCha20-Poly1305)
- Cross-platform: Linux (Wayland/X11), Windows, WSL
- Simple CLI interface
- No cloud, no accounts - direct P2P over LAN

## Installation

### From Source

```bash
# Requires Rust toolchain
cargo install --path crates/cb-cli

# Or build manually
cargo build --release
# Binary at target/release/cb-sync
```

### NixOS

```bash
nix-shell  # Enter dev environment
cargo build --release
```

## Quick Start

```bash
# On receiving machine - start listener
cb-sync -p 'secret' listen

# On sending machine - send clipboard
cb-sync -p 'secret' send <TARGET_IP>

# Or send specific text
cb-sync -p 'secret' send <TARGET_IP> "Hello, World!"
```

## Usage

### Basic Commands

```bash
# Local clipboard operations
cb-sync copy "text"     # Copy to clipboard
cb-sync paste           # Print clipboard contents

# Remote sync (encrypted)
cb-sync -p 'password' send <host>      # Send clipboard
cb-sync -p 'password' receive          # Receive once
cb-sync -p 'password' listen           # Continuous receive

# Image sync
cb-sync send <host> --image            # Send clipboard image
cb-sync send <host> -f image.png       # Send image file
cb-sync receive -o received.png        # Save received image

# Environment info
cb-sync info
```

### Encryption

All network communication should be encrypted. Two options:

```bash
# Option 1: Password-based
cb-sync -p 'your-password' send <host>

# Option 2: Key-based (more secure)
cb-sync keygen                         # Generate key
cb-sync -k 'base64-key' send <host>

# Environment variables also work
export CB_SYNC_PASSWORD='password'
# or
export CB_SYNC_KEY='base64-key'
```

### Configuration File

Create a config file at `~/.config/cb-sync/config.toml`:

```bash
cb-sync config init   # Create template
cb-sync config show   # Show current config
cb-sync config path   # Show config file path
```

Example configuration:

```toml
[general]
port = 34812
timeout_secs = 20

[encryption]
password = "shared-secret"
# or: key = "base64-encoded-key"

[targets]
default = "<TARGET_IP>"
desktop = "<HOME_IP>"
laptop = "<LAPTOP_IP>"
```

Use named targets:

```bash
cb-sync send @desktop   # Uses <HOME_IP>
cb-sync send @laptop    # Uses <LAPTOP_IP>
```

## Platform Support

| Platform | Status |
|----------|--------|
| Linux (Wayland) | Supported |
| Linux (X11) | Supported |
| Windows | Supported |
| WSL | Supported |
| Android | Planned |

## Architecture

```
cb-sync/
├── crates/
│   ├── cb-core/         # Core library
│   │   ├── clipboard.rs # Platform clipboard abstraction
│   │   ├── config.rs    # Configuration file handling
│   │   ├── crypto.rs    # ChaCha20-Poly1305 encryption
│   │   ├── protocol.rs  # Message types (JSON)
│   │   └── sync.rs      # TCP send/receive
│   └── cb-cli/          # CLI application
└── docs/
    └── PLAN.md          # Development roadmap
```

## Technical Details

- **Protocol**: TCP on port 34812 (configurable)
- **Encryption**: ChaCha20-Poly1305 (AEAD)
- **Message Format**: JSON
- **Timeout**: 20 seconds (configurable)

### Message Types

```json
// Text
{"type":"clipboard","text":"content","timestamp":1234567890}

// Image (base64 encoded)
{"type":"image","data":"base64...","timestamp":1234567890}

// Encrypted (wraps any message)
{"type":"encrypted","data":"base64-ciphertext","content_type":"text","timestamp":1234567890}

// Acknowledgment
{"type":"ack"}
```

## Security

- **Explicit activation**: No background daemon - you control when to sync
- **Encryption**: ChaCha20-Poly1305 for all network traffic
- **LAN only**: No internet connectivity required
- **20s timeout**: Listener auto-closes to minimize exposure

## Future Roadmap

- [ ] OS path translation (Linux `~` <-> Windows `%USERPROFILE%`)
- [ ] Android support
- [ ] AI-powered clipboard transformation pipeline

## License

MIT
