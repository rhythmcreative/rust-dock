<div align="center">

<img width="200" height="200" alt="rust-dock" src="https://github.com/user-attachments/assets/794946b8-8976-4b6e-8268-fc09940a5473" />

</div>

<h1 align="center">rust-dock</h1>

<div align="center">

[![GTK4](https://img.shields.io/badge/GTK4-f5c2e7?style=for-the-badge "GTK4 - The Toolkit for Creating Graphical User Interfaces")](https://www.gtk.org/)
[![Rust](https://img.shields.io/badge/Rust-f38ba8?style=for-the-badge "Rust - Systems programming language")](https://www.rust-lang.org/)
[![Cargo](https://img.shields.io/badge/Cargo-fab387?style=for-the-badge "Cargo - The Rust package manager")](https://doc.rust-lang.org/cargo/)
[![Hyprland](https://img.shields.io/badge/Hyprland-abd6fd?style=for-the-badge "Hyprland - A dynamic tiling Wayland compositor based on wlroots that doesn't sacrifice on its looks")](https://hyprland.org/)
[![grim](https://img.shields.io/badge/grim-94e2d5?style=for-the-badge "grim - Grab images from a Wayland compositor for window-preview thumbnails")](https://git.sr.ht/~emersion/grim)

</div>

<div align="center">
 <p><i>A modern, high-performance dock for Hyprland built with Rust and GTK4.</i></p>
</div>

[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=PREVIEW)](https://git.io/typing-svg)



## Overview

rust-dock is a lightweight taskbar and application launcher designed specifically for the
Hyprland Wayland compositor. It uses GTK4 with `gtk4-layer-shell` for hardware-accelerated
rendering and native Wayland integration, and talks to Hyprland directly over its IPC socket
for instant, event-driven updates.

## Features

- **Live window previews** — hovering an icon shows a panel of rounded thumbnail cards, one
  per open window, captured live with `grim`. Click a card to focus that window, or the `×`
  to close it.
- **Click to focus or launch** — clicking a running app focuses its window (and cycles
  through them on repeated clicks); clicking a closed app launches it.
- **Active-window highlight** — the icon of the focused app is highlighted in real time.
- **Right-click context menu** — open a new window, pin/unpin the app, or close all of its
  windows, with the app name and icon shown in a clean header.
- **Drag & drop** — reorder pinned icons by dragging them; the new order is saved.
- **Real-time Pywal sync** — automatically reloads colors from
  `~/.cache/wal/colors-waybar.css` the moment they change, no restart needed.
- **Config hot-reload** — edits to the config file are applied live (layout-affecting
  options like position still need a restart).
- **Hyprland task management** — running applications appear and disappear in real time,
  driven by Hyprland's event socket.
- **Pinned applications** — keep favourite apps on the dock; managed from the right-click
  menu or a simple text file.
- **Smart view (auto-hide)** — optionally drop the dock below other windows and reveal it
  when the cursor reaches the screen edge.
- **Signal control** — toggle or force-show the dock with Unix signals.
- **CLI + config file** — configure via command-line flags or an INI file (flags win).
- **Multi-position & multi-monitor** — anchor to any edge and target a specific output.

## Installation

### Prerequisites

The install script installs dependencies automatically for Arch, Fedora, and Debian/Ubuntu.
You will need:

- GTK4 development libraries
- `gtk4-layer-shell` development libraries
- Rust and Cargo (system packages or [rustup](https://rustup.rs/))
- Hyprland
- `grim` (for window-preview thumbnails)

### Install

From the project root:

```bash
./install.sh
```

The script will:

1. Detect your distribution and install the required dependencies.
2. Build the project in release mode.
3. Install the binary to `~/.local/bin/rust-dock`.
4. Create a default configuration in `~/.config/rust-dock/`.

> **Note:** make sure `~/.local/bin` is on your `PATH`.

## Usage

Start the dock:

```bash
rust-dock
```

To launch it automatically with Hyprland, add this to `~/.config/hypr/hyprland.conf`:

```ini
exec-once = ~/.local/bin/rust-dock --exclusive-zone --position bottom --icon-size 32
```

### Command-line options

Flags override the config file.

| Flag | Description | Default |
| --- | --- | --- |
| `-p, --position <POS>` | `top` \| `bottom` \| `left` \| `right` | `bottom` |
| `-i, --icon-size <PX>` | Icon size in pixels | `32` |
| `--padding <PX>` | Internal padding | `4` |
| `--spacing <PX>` | Space between icons | `5` |
| `--radius <PX>` | Corner radius | `10` |
| `--opacity <0.0-1.0>` | Background opacity | `0.8` |
| `-o, --output <NAME>` | Target a specific monitor (e.g. `DP-6`) | auto |
| `-l, --launcher-command <CMD>` | Command for the launcher button | — |
| `--launcher` / `--no-launcher` | Show / hide the launcher button | hidden |
| `-e, --exclusive-zone` / `--no-exclusive-zone` | Reserve screen space (or not) | reserve |
| `--smart-view` | Auto-hide until the cursor hits the edge | off |
| `--style <PATH>` | Extra CSS file appended to the stylesheet | — |

Run `rust-dock --help` for the full list.

### Remote control

| Action       | Command                  |
| ------------ | ------------------------ |
| Toggle dock  | `pkill -USR1 rust-dock`  |
| Force show   | `pkill -USR2 rust-dock`  |

## Configuration

rust-dock is configured through an INI file at **`~/.config/rust-dock/hypr-dock.conf`**
(created by the installer). Changes are picked up the next time the dock starts.

```ini
[General]
CurrentTheme    = lotos      ; theme folder under ~/.config/rust-dock/themes/
Position        = bottom     ; top | bottom | left | right
IconSize        = 32         ; icon size in pixels
Padding         = 4          ; internal padding
Radius          = 10         ; corner radius
Opacity         = 0.8        ; background opacity (0.0 - 1.0)
Exclusive       = true       ; reserve space so windows don't overlap the dock
SmartView       = false      ; auto-hide the dock until the cursor hits the edge
AutoHideDelay   = 400        ; smart-view hide delay (ms)
SystemGapUsed   = true       ; use Hyprland's general:gaps_out as the dock margin
Margin          = 8          ; dock margin when SystemGapUsed = false
Output          =            ; target monitor connector (e.g. DP-6); empty = auto
LauncherCommand =            ; command for the launcher button
NoLauncher      = true       ; hide the launcher button
ContextPos      = 5

[General.preview]
Mode       = none           ; window-preview mode
FPS        = 30
BufferSize = 5
ShowDelay  = 70             ; delay before a preview appears (ms)
HideDelay  = 300            ; delay before a preview hides (ms)
MoveDelay  = 30             ; delay when moving between previews (ms)

[Theme]
Spacing = 5                 ; spacing between dock icons
```

Edits to this file are applied live, except layout-affecting options (position,
exclusive zone, margins) which take effect on the next start.

### Pinned applications

Pinned apps are stored, one desktop-entry id per line, in
**`~/.local/share/rust-dock/pinned`**:

```
firefox
kitty
code
```

The id is the `.desktop` file name without the extension (e.g. `firefox` for
`firefox.desktop`). You normally don't edit this by hand — just **right-click an icon and
choose "Pin to taskbar" / "Unpin from taskbar"**, and **drag pinned icons** to reorder them.

### Theming

Colors follow your Pywal palette automatically. For extra tweaks, drop a `style.css` in
`~/.config/rust-dock/themes/<CurrentTheme>/style.css`; it is appended on top of the built-in
stylesheet.

## Uninstallation

```bash
./uninstall.sh
```

This stops any running instance, removes the binary, and (after confirmation) removes the
configuration and data directories.

## License

See the repository for license details.
