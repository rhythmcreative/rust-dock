#!/usr/bin/env bash

set -euo pipefail

echo "Starting rust-dock installation..."

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# ── Dependencies ────────────────────────────────────────────────────────────
echo "Checking and installing dependencies..."

if command_exists pacman; then
    echo "Detected Arch Linux (pacman)."
    if command_exists yay; then
        yay -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel
        command_exists cargo || yay -S --needed --noconfirm rust
    else
        sudo pacman -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel
        command_exists cargo || sudo pacman -S --needed --noconfirm rust
    fi
elif command_exists dnf; then
    echo "Detected Fedora/RHEL (dnf)."
    sudo dnf install -y gtk4-devel gtk4-layer-shell-devel grim pkgconf-pkg-config gcc gcc-c++ rust cargo
elif command_exists apt; then
    echo "Detected Debian/Ubuntu (apt)."
    sudo apt update
    sudo apt install -y libgtk-4-dev libgtk4-layer-shell-dev grim pkg-config build-essential cargo rustc
else
    echo "Warning: no supported package manager found (pacman/yay, dnf, apt)."
    echo "Please install GTK4, gtk4-layer-shell, grim, and Rust/Cargo manually."
fi

# Fall back to rustup if cargo is still missing.
if ! command_exists cargo; then
    echo "Cargo not found. Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

# ── Build ───────────────────────────────────────────────────────────────────
echo "Building rust-dock in release mode..."
cargo build --release

# ── Install binary ────────────────────────────────────────────────────────────
BIN_DIR="$HOME/.local/bin"
echo "Installing binary to $BIN_DIR/rust-dock..."
mkdir -p "$BIN_DIR"
install -m 755 target/release/rust-dock "$BIN_DIR/rust-dock"

# ── Default configuration ─────────────────────────────────────────────────────
CONFIG_DIR="$HOME/.config/rust-dock"
DATA_DIR="$HOME/.local/share/rust-dock"
CONFIG_FILE="$CONFIG_DIR/hypr-dock.conf"
PINNED_FILE="$DATA_DIR/pinned"

echo "Setting up default configuration..."
mkdir -p "$CONFIG_DIR" "$DATA_DIR"

if [ ! -f "$CONFIG_FILE" ]; then
    cat > "$CONFIG_FILE" <<'EOF'
[General]
CurrentTheme  = lotos
Position      = bottom
IconSize      = 32
Exclusive     = true
SmartView     = false
AutoHideDelay = 400
SystemGapUsed = true
Margin        = 8
ContextPos    = 5

[General.preview]
Mode       = none
FPS        = 30
BufferSize = 5
ShowDelay  = 500
HideDelay  = 350
MoveDelay  = 100

[Theme]
Spacing = 5
EOF
    echo "  Created $CONFIG_FILE"
else
    echo "  Keeping existing $CONFIG_FILE"
fi

if [ ! -f "$PINNED_FILE" ]; then
    printf 'firefox\nkitty\ncode\n' > "$PINNED_FILE"
    echo "  Created $PINNED_FILE"
else
    echo "  Keeping existing $PINNED_FILE"
fi

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
echo "Installation complete."
echo "-------------------------------------------------------"
echo "Start rust-dock with:"
echo "  $BIN_DIR/rust-dock"
echo ""
echo "Add it to Hyprland (~/.config/hypr/hyprland.conf):"
echo "  exec-once = $BIN_DIR/rust-dock"
echo ""
echo "Edit your config at:"
echo "  $CONFIG_FILE"
echo "-------------------------------------------------------"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Note: $BIN_DIR is not on your PATH. Add it to use 'rust-dock' directly." ;;
esac
