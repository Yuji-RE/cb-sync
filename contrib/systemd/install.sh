#!/bin/bash
# Install cb-sync systemd user service
#
# Usage: ./install.sh
#
# This script installs cb-sync as a systemd user service.
# After installation, enable and start with:
#   systemctl --user enable cb-sync
#   systemctl --user start cb-sync

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_FILE="cb-sync.service"
SERVICE_DIR="$HOME/.config/systemd/user"

echo "Installing cb-sync systemd user service..."

# Create systemd user directory if needed
mkdir -p "$SERVICE_DIR"

# Copy service file
cp "$SCRIPT_DIR/$SERVICE_FILE" "$SERVICE_DIR/"

# Reload systemd daemon
systemctl --user daemon-reload

echo ""
echo "Service installed successfully!"
echo ""
echo "Before starting, ensure you have:"
echo "  1. cb-sync binary in PATH (cargo install --path crates/cb-cli)"
echo "  2. Config file with encryption and peers (~/.config/cb-sync/config.toml)"
echo ""
echo "To enable and start:"
echo "  systemctl --user enable cb-sync"
echo "  systemctl --user start cb-sync"
echo ""
echo "To check status:"
echo "  systemctl --user status cb-sync"
echo ""
echo "To view logs:"
echo "  journalctl --user -u cb-sync -f"
