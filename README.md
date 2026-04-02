# cb-sync

Cross-platform clipboard synchronization daemon written in Rust.

## Overview

Automatic, encrypted clipboard sync between your devices over LAN. Copy on one machine, paste on all others within seconds.

**Key Features:**
- Automatic bidirectional sync (daemon mode)
- End-to-end encryption (ChaCha20-Poly1305)
- Cross-platform: Linux (Wayland/X11), Windows, WSL
- No cloud, no accounts - direct P2P over LAN
- systemd integration for auto-start

## Quick Start

### 1. Install

```bash
cargo install --path crates/cb-cli
# or
cargo build --release && cp target/release/cb-sync ~/.cargo/bin/
```

### 2. Configure

```bash
cb-sync config init
```

Edit `~/.config/cb-sync/config.toml`:

```toml
[encryption]
password = "<YOUR_PASSWORD>"

[targets]
laptop = "<LAPTOP_IP>"
desktop = "<DESKTOP_IP>"

[daemon]
peers = ["laptop", "desktop"]
```

### 3. Run on All Devices

```bash
cb-sync daemon
```

That's it. Copy on any device, paste on all others.

## Daemon Mode

The daemon monitors your clipboard and syncs changes with configured peers automatically.

```bash
cb-sync daemon           # Start daemon
cb-sync daemon -vv       # With verbose logging
cb-sync daemon --foreground  # For debugging
```

### Configuration Options

```toml
[daemon]
peers = ["laptop", "desktop"]  # Targets to sync with
poll_interval_ms = 500         # How often to check clipboard (default: 500)
sync_cooldown_ms = 2000        # Prevent sync loops (default: 2000)
max_retries = 3                # Retry failed sends (default: 3)
```

### systemd Service (Linux)

Run daemon automatically on login:

```bash
# Install service
bash contrib/systemd/install.sh

# Enable and start
systemctl --user enable --now cb-sync

# Check status / logs
systemctl --user status cb-sync
journalctl --user -u cb-sync -f
```

## Platform Setup

| Platform | Status |
|----------|--------|
| Linux (Wayland) | Supported |
| Linux (X11) | Supported |
| Windows | Supported |
| WSL | Supported |
| Android | Planned |

### Firewall

Allow TCP port 34812:

```bash
# Linux (ufw)
sudo ufw allow 34812/tcp

# Linux (firewalld)
sudo firewall-cmd --add-port=34812/tcp --permanent && sudo firewall-cmd --reload
```

```powershell
# Windows (PowerShell as Admin)
New-NetFirewallRule -DisplayName "cb-sync" -Direction Inbound -Protocol TCP -LocalPort 34812 -Action Allow
```

### WSL Network Setup

WSL2 uses NAT, so peers connect to your **Windows IP** (not WSL internal IP). Set up port forwarding:

```powershell
# PowerShell as Admin - get WSL IP first: wsl -- ip addr show eth0
netsh interface portproxy add v4tov4 listenport=34812 listenaddress=0.0.0.0 connectport=34812 connectaddress=<WSL_IP>
```

On peer machines, use Windows host IP:

```toml
[targets]
wsl-machine = "<WINDOWS_HOST_IP>"
```

## Security

- **Encryption required**: Daemon mode rejects plaintext
- **ChaCha20-Poly1305**: Authenticated encryption (AEAD)
- **Loop prevention**: Won't re-broadcast received content
- **LAN only**: No internet required

### Recommendations

**Use `keygen` instead of password** (more secure):
```bash
cb-sync keygen  # Generate random 256-bit key
```

Then use in config:
```toml
[encryption]
key = "<GENERATED_KEY>"  # Instead of password
```

Password-based keys use a fixed salt for convenience, making them weaker against offline attacks. Generated keys are cryptographically random and recommended for higher security.

**Bind address**: Default is `0.0.0.0` (all interfaces). On multi-homed systems or VPNs, consider binding to a specific LAN interface:
```bash
cb-sync daemon --bind 192.168.1.100
```

**Gaming**: Some games (e.g., Apex Legends) may experience server connection issues while the daemon is running. Stop the daemon before playing:
```bash
systemctl --user stop cb-sync   # Linux
pkill -x cb-sync                # WSL/manual
```

## Manual Commands

For one-off transfers without running the daemon:

```bash
# Send clipboard to specific target
cb-sync -p '<PASSWORD>' send <TARGET_IP>
cb-sync send @laptop  # Using named target from config

# Receive mode
cb-sync -p '<PASSWORD>' listen   # Continuous
cb-sync -p '<PASSWORD>' receive  # Once

# Local clipboard
cb-sync copy "text"   # Copy to clipboard
cb-sync paste         # Print clipboard

# Images
cb-sync send <TARGET> --image        # Send clipboard image
cb-sync send <TARGET> -f image.png   # Send file
cb-sync receive -o output.png        # Save received image

# Key management
cb-sync keygen  # Generate encryption key
```

## Architecture

```
cb-sync/
├── crates/
│   ├── cb-core/         # Core library
│   │   ├── clipboard.rs # Platform clipboard abstraction
│   │   ├── config.rs    # Configuration handling
│   │   ├── crypto.rs    # ChaCha20-Poly1305 encryption
│   │   ├── daemon.rs    # Auto-sync daemon
│   │   └── sync.rs      # TCP send/receive
│   └── cb-cli/          # CLI application
└── contrib/
    └── systemd/         # systemd service files
```

## Technical Details

- **Protocol**: TCP on port 34812
- **Encryption**: ChaCha20-Poly1305 (AEAD)
- **Key derivation**: Argon2id (password-based)
- **Message format**: JSON

---

## My Setup

NixOS + WSL configuration for reference.

### NixOS

**Config** (`~/.config/cb-sync/config.toml`):
```toml
[encryption]
password = "<PASSWORD>"

[targets]
wsl = "<WINDOWS_IP>"

[daemon]
peers = ["wsl"]
```

**systemd service**:
```bash
bash contrib/systemd/install.sh
systemctl --user enable --now cb-sync
```

**Failure notifications** (add `libnotify` to NixOS packages):
```nix
environment.systemPackages = with pkgs; [ libnotify ];
```

### WSL

**Config** (`~/.config/cb-sync/config.toml`):
```toml
[encryption]
password = "<PASSWORD>"

[targets]
nixos = "<NIXOS_IP>"

[daemon]
peers = ["nixos"]
```

**Windows port forwarding** (PowerShell Admin):
```powershell
# Get WSL IP
wsl -- ip addr show eth0 | findstr "inet "

# Add port forward
netsh interface portproxy add v4tov4 listenport=34812 listenaddress=0.0.0.0 connectport=34812 connectaddress=<WSL_IP>
```

**Windows firewall**:
```powershell
New-NetFirewallRule -DisplayName "cb-sync" -Direction Inbound -Protocol TCP -LocalPort 34812 -Action Allow
```

**Run daemon**:
```bash
cb-sync daemon -vv
```

### Useful Commands

```bash
# Check daemon status
systemctl --user status cb-sync

# View logs
journalctl --user -u cb-sync -f

# Stop daemon
systemctl --user stop cb-sync

# Restart daemon
systemctl --user restart cb-sync
```

---

## License

MIT
