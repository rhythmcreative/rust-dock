# rust-dock

A modern, high-performance dock for Hyprland built with Rust and GTK4.

## Overview

rust-dock is a lightweight taskbar and application launcher designed specifically for the Hyprland Wayland compositor. It leverages GTK4 for hardware-accelerated rendering and provides seamless integration with the Hyprland ecosystem.

## Key Features

- **Modern Architecture**: Built with GTK4 and gtk4-layer-shell for superior performance and native Wayland support.
- **Real-time Pywal Sync**: Automatically detects and applies color changes from Pywal (~/.cache/wal/colors-waybar.css) without restarting.
- **Task Management**: Displays running applications with real-time updates from Hyprland.
- **Pinned Applications**: Support for favorite applications via a simple JSON configuration.
- **Extensive CLI Customization**: Configure icon size, padding, spacing, corner radius, and opacity directly from the command line.
- **Signal Support**: Control visibility using Unix signals (SIGUSR1 for toggle, SIGUSR2 for force show).
- **Multi-Monitor Support**: Ability to target specific monitors using connector names.

## Installation

### Prerequisites

The installation script handles dependencies for Arch Linux, Fedora, and Debian/Ubuntu.

- GTK4 development libraries
- gtk4-layer-shell development libraries
- Rust and Cargo (installed via system or rustup)
- Hyprland

### Using the Install Script

Run the following command from the project root:

```bash
./install.sh
```

This script will:
1. Detect your distribution and install necessary dependencies.
2. Build the project in release mode.
3. Install the binary to ~/.local/bin/rust-dock.
4. Set up a default configuration directory.

## Usage

Start the dock with your preferred settings:

```bash
rust-dock --icon-size 32 --padding 6 --spacing 8 --radius 12 --opacity 0.7 --exclusive-zone --position bottom
```

### Command Line Options

- `-p, --position`: Set dock position (top, bottom, left, right). Default: bottom.
- `-i, --icon-size`: Set icon size in pixels. Default: 32.
- `--padding`: Internal padding of the dock. Default: 4.
- `--spacing`: Space between application icons. Default: 6.
- `--radius`: Corner radius of the dock. Default: 10.
- `--opacity`: Background opacity (0.0 to 1.0). Default: 0.8.
- `-e, --exclusive-zone`: Move other windows to avoid overlap.
- `-o, --output`: Specify a monitor name (e.g., DP-6).
- `-l, --launcher-command`: Command for the launcher button. Default: nwg-drawer.
- `--no-launcher`: Disable the launcher button.

### Hyprland Integration

Add the following to your ~/.config/hypr/hyprland.conf:

```ini
exec-once = ~/.local/bin/rust-dock --exclusive-zone --position bottom --icon-size 32
```

### Remote Control

- Toggle visibility: `pkill -USR1 rust-dock`
- Force show: `pkill -USR2 rust-dock`

## Configuration

Pinned applications are managed in `~/.config/rust-dock/config.json`:

```json
{
  "pinned_apps": [
    "firefox",
    "kitty",
    "code"
  ]
}
```

## Uninstallation

To remove rust-dock and its configuration, run:

```bash
./uninstall.sh
```
