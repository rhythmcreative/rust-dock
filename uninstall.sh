#!/bin/bash

echo "Uninstalling rust-dock..."

if pgrep -x "rust-dock" > /dev/null; then
    echo "Stopping running instances..."
    pkill -x rust-dock || true
fi

if [ -f "$HOME/.local/bin/rust-dock" ]; then
    echo "Removing binary from $HOME/.local/bin..."
    rm -f "$HOME/.local/bin/rust-dock"
fi

if [ -d "$HOME/.config/rust-dock" ]; then
    read -p "Do you want to remove the configuration directory (~/.config/rust-dock)? [y/N] " confirm
    if [[ "$confirm" =~ ^[Yy]$ ]]; then
        echo "Removing configuration..."
        rm -rf "$HOME/.config/rust-dock"
    fi
fi

echo "Uninstallation complete."
