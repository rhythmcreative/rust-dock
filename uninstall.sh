#!/usr/bin/env bash

set -euo pipefail

echo "Uninstalling rust-dock..."

# ── Stop running instances ────────────────────────────────────────────────────
if pgrep -x rust-dock >/dev/null; then
    echo "Stopping running instances..."
    pkill -x rust-dock || true
fi

# ── Remove binary ─────────────────────────────────────────────────────────────
BIN="$HOME/.local/bin/rust-dock"
if [ -f "$BIN" ]; then
    echo "Removing binary $BIN..."
    rm -f "$BIN"
fi

# ── Remove leftover preview thumbnails ────────────────────────────────────────
rm -f /tmp/rust-dock-preview-*.png 2>/dev/null || true

# ── Remove config + data (with confirmation) ──────────────────────────────────
CONFIG_DIR="$HOME/.config/rust-dock"
DATA_DIR="$HOME/.local/share/rust-dock"

if [ -d "$CONFIG_DIR" ] || [ -d "$DATA_DIR" ]; then
    read -r -p "Also remove configuration and pinned apps ($CONFIG_DIR, $DATA_DIR)? [y/N] " confirm
    if [[ "$confirm" =~ ^[Yy]$ ]]; then
        echo "Removing configuration and data..."
        rm -rf "$CONFIG_DIR" "$DATA_DIR"
    else
        echo "Keeping configuration and data."
    fi
fi

echo "Uninstallation complete."
