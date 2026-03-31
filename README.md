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
cb-sync -p '<PASSWORD>' listen

# On sending machine - send clipboard
cb-sync -p '<PASSWORD>' send <RECEIVER_IP>

# Or send specific text
cb-sync -p '<PASSWORD>' send <RECEIVER_IP> "Hello, World!"
```

## Usage

### Basic Commands

```bash
# Local clipboard operations
cb-sync copy "text"     # Copy to clipboard
cb-sync paste           # Print clipboard contents

# Remote sync (encrypted)
cb-sync -p '<PASSWORD>' send <RECEIVER_IP>   # Send clipboard
cb-sync -p '<PASSWORD>' receive              # Receive once
cb-sync -p '<PASSWORD>' listen               # Continuous receive

# Image sync
cb-sync send <RECEIVER_IP> --image           # Send clipboard image
cb-sync send <RECEIVER_IP> -f <IMAGE_FILE>   # Send image file
cb-sync receive -o <OUTPUT_FILE>             # Save received image

# Environment info
cb-sync info
```

### Encryption

All network communication should be encrypted. Two options:

```bash
# Option 1: Password-based
cb-sync -p '<PASSWORD>' send <RECEIVER_IP>

# Option 2: Key-based (more secure)
cb-sync keygen                              # Generate key
cb-sync -k '<BASE64_KEY>' send <RECEIVER_IP>

# Environment variables also work
export CB_SYNC_PASSWORD='<PASSWORD>'
# or
export CB_SYNC_KEY='<BASE64_KEY>'
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
password = "<PASSWORD>"
# or: key = "<BASE64_KEY>"

[targets]
default = "<DEFAULT_TARGET_IP>"
desktop = "<DESKTOP_IP>"
laptop = "<LAPTOP_IP>"
```

Use named targets:

```bash
cb-sync send @desktop   # Uses <DESKTOP_IP>
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

### WSL Network Setup

WSL2 uses NAT networking, so peers need to connect to your **Windows IP**, not the WSL internal IP. Port forwarding is required:

```powershell
# Run as Administrator on Windows
# Get WSL IP (run in WSL terminal first: ip addr show eth0)
netsh interface portproxy add v4tov4 listenport=34812 listenaddress=0.0.0.0 connectport=34812 connectaddress=<WSL_IP>

# Verify
netsh interface portproxy show all

# To remove later
netsh interface portproxy delete v4tov4 listenport=34812 listenaddress=0.0.0.0
```

On the peer machine, use the Windows host IP as the target:

```toml
# ~/.config/cb-sync/config.toml (on peer)
[targets]
wsl = "<WINDOWS_HOST_IP>"
```

## Setup

### Windows Firewall Configuration

To receive clipboard data on Windows, you need to allow incoming connections on port 34812.

**Option 1: PowerShell (Administrator)**

```powershell
# Allow incoming TCP connections
New-NetFirewallRule -DisplayName "cb-sync" -Direction Inbound -Protocol TCP -LocalPort 34812 -Action Allow

# Verify the rule
Get-NetFirewallRule -DisplayName "cb-sync"
```

**Option 2: netsh (Administrator)**

```cmd
:: Allow incoming TCP connections
netsh advfirewall firewall add rule name="cb-sync" dir=in action=allow protocol=tcp localport=34812

:: Verify the rule
netsh advfirewall firewall show rule name="cb-sync"

:: To remove the rule later
netsh advfirewall firewall delete rule name="cb-sync"
```

### Linux Firewall (if enabled)

```bash
# ufw
sudo ufw allow 34812/tcp

# firewalld
sudo firewall-cmd --add-port=34812/tcp --permanent
sudo firewall-cmd --reload
```

## Architecture

```
cb-sync/
├── crates/
│   ├── cb-core/         # Core library
│   │   ├── clipboard.rs # Platform clipboard abstraction
│   │   ├── config.rs    # Configuration file handling
│   │   ├── crypto.rs    # ChaCha20-Poly1305 encryption
│   │   ├── daemon.rs    # Auto-sync daemon
│   │   ├── protocol.rs  # Message types (JSON)
│   │   └── sync.rs      # TCP send/receive
│   └── cb-cli/          # CLI application
├── contrib/
│   └── systemd/         # systemd service files
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

## Daemon Mode (Auto-sync)

For automatic bidirectional clipboard sync, use daemon mode:

```bash
# Start daemon (requires encryption)
cb-sync daemon

# With verbose logging
cb-sync daemon -vv
```

### Daemon Configuration

Add to `~/.config/cb-sync/config.toml`:

```toml
[encryption]
password = "<PASSWORD>"

[targets]
desktop = "<DESKTOP_IP>"
laptop = "<LAPTOP_IP>"

[daemon]
peers = ["desktop", "laptop"]
poll_interval_ms = 500       # Clipboard check interval
sync_cooldown_ms = 2000      # Loop prevention cooldown
```

### systemd Service (Linux)

```bash
# Install user service
cd contrib/systemd && ./install.sh

# Enable and start
systemctl --user enable cb-sync
systemctl --user start cb-sync

# View logs
journalctl --user -u cb-sync -f
```

## Security

- **Encryption required in daemon mode**: Plaintext sync is rejected
- **Explicit activation**: Manual commands give you full control
- **ChaCha20-Poly1305**: Authenticated encryption for all traffic
- **LAN only**: No internet connectivity required
- **20s timeout**: Listener auto-closes to minimize exposure
- **Loop prevention**: Received content is not re-broadcast

## Future Roadmap

- [ ] OS path translation (Linux `~` <-> Windows `%USERPROFILE%`)
- [ ] Android support
- [ ] AI-powered clipboard transformation pipeline

## License

MIT
