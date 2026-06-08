#!/bin/bash

set -e

echo "Starting rust-dock installation..."

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

echo "Checking and installing dependencies..."

if command_exists yay; then
    echo "Detected yay (Arch Linux). Installing dependencies..."
    yay -S --needed --noconfirm gtk4 gtk4-layer-shell pkgconf base-devel
    if ! command_exists cargo; then
        yay -S --needed --noconfirm rust cargo
    else
        echo "Rust/Cargo already installed, skipping..."
    fi
elif command_exists pacman; then
    echo "Detected pacman (Arch Linux). Installing dependencies..."
    sudo pacman -S --needed --noconfirm gtk4 gtk4-layer-shell pkgconf base-devel
    if ! command_exists cargo; then
        sudo pacman -S --needed --noconfirm rust cargo
    else
        echo "Rust/Cargo already installed, skipping..."
    fi
elif command_exists dnf; then
    echo "Detected dnf (Fedora/RHEL). Installing dependencies..."
    sudo dnf install -y gtk4-devel gtk4-layer-shell-devel pkgconf-pkg-config gcc gcc-c++ rust cargo
elif command_exists apt; then
    echo "Detected apt (Debian/Ubuntu). Installing dependencies..."
    sudo apt update
    sudo apt install -y libgtk-4-dev libgtk4-layer-shell-dev pkg-config build-essential cargo rustc
else
    echo "Warning: No supported package manager found (apt, dnf, pacman/yay). Please ensure dependencies are installed manually."
fi

if ! command_exists cargo; then
    echo "Cargo not found in system paths. Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "Building rust-dock in release mode..."
cargo build --release

echo "Installing binary to $HOME/.local/bin/rust-dock..."
mkdir -p "$HOME/.local/bin"
cp target/release/rust-dock "$HOME/.local/bin/rust-dock"
chmod +x "$HOME/.local/bin/rust-dock"

echo "Setting up default configuration..."
mkdir -p "$HOME/.config/rust-dock"
if [ ! -f "$HOME/.config/rust-dock/config.json" ]; then
    echo '{"pinned_apps": ["firefox", "kitty", "code"]}' > "$HOME/.config/rust-dock/config.json"
fi

echo ""
echo "Installation complete."
echo "-------------------------------------------------------"
echo "To start rust-dock, run:"
echo "  ~/.local/bin/rust-dock --exclusive-zone --position bottom"
echo ""
echo "To add it to your Hyprland config (~/.config/hypr/hyprland.conf):"
echo "  exec-once = $HOME/.local/bin/rust-dock --exclusive-zone --position bottom"
echo "-------------------------------------------------------"
