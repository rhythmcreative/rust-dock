#!/usr/bin/env bash
# rust-dock installer — supports most major Linux distributions.

set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()    { echo -e "${GREEN}[+]${NC} $*"; }
warn()    { echo -e "${YELLOW}[!]${NC} $*"; }
error()   { echo -e "${RED}[✗]${NC} $*" >&2; }
header()  { echo -e "\n${BOLD}$*${NC}"; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

header "rust-dock installer"

# ── 1. Dependencies ──────────────────────────────────────────────────────────
header "Installing dependencies..."

install_deps_pacman() {
    local pm="pacman"
    command_exists yay  && pm="yay"
    command_exists paru && pm="paru"
    info "Package manager: $pm (Arch / Manjaro / EndeavourOS)"
    if [[ "$pm" == "pacman" ]]; then
        sudo pacman -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel
        command_exists cargo || sudo pacman -S --needed --noconfirm rust
    else
        $pm -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel
        command_exists cargo || $pm -S --needed --noconfirm rust
    fi
}

install_deps_apt() {
    info "Package manager: apt (Debian / Ubuntu / Linux Mint / Pop!_OS)"
    sudo apt-get update -qq
    # gtk4-layer-shell may be named differently on older distros
    local layer_pkg="libgtk4-layer-shell-dev"
    apt-cache show "$layer_pkg" >/dev/null 2>&1 || layer_pkg="libgtk4-layer-shell1-dev"
    sudo apt-get install -y libgtk-4-dev "$layer_pkg" grim pkg-config \
        build-essential libglib2.0-dev libcairo2-dev
    command_exists cargo || sudo apt-get install -y cargo rustc
}

install_deps_dnf() {
    info "Package manager: dnf (Fedora / RHEL / CentOS Stream)"
    sudo dnf install -y gtk4-devel gtk4-layer-shell-devel grim \
        pkgconf-pkg-config gcc gcc-c++ rust cargo
}

install_deps_zypper() {
    info "Package manager: zypper (openSUSE Leap / Tumbleweed)"
    sudo zypper install -y gtk4-devel gtk4-layer-shell-devel grim \
        pkg-config gcc gcc-c++ rust cargo
}

install_deps_xbps() {
    info "Package manager: xbps (Void Linux)"
    sudo xbps-install -Sy gtk4-devel gtk4-layer-shell-devel grim \
        pkg-config gcc rust cargo
}

install_deps_apk() {
    info "Package manager: apk (Alpine Linux)"
    sudo apk add --no-cache gtk4.0-dev grim pkgconf build-base rust cargo
    # gtk4-layer-shell may need to be compiled from source on Alpine
    if ! pkg-config --exists gtk4-layer-shell 2>/dev/null; then
        warn "gtk4-layer-shell not in apk — installing from source..."
        install_layer_shell_from_source
    fi
}

install_deps_emerge() {
    info "Package manager: emerge (Gentoo)"
    sudo emerge --noreplace x11-libs/gtk+ dev-libs/gtk4-layer-shell \
        x11-apps/grim dev-util/pkgconf virtual/rust
}

install_deps_eopkg() {
    info "Package manager: eopkg (Solus)"
    sudo eopkg install -y libgtk-4-devel grim pkg-config gcc rust cargo
    if ! pkg-config --exists gtk4-layer-shell 2>/dev/null; then
        warn "gtk4-layer-shell not in eopkg — installing from source..."
        install_layer_shell_from_source
    fi
}

install_layer_shell_from_source() {
    info "Building gtk4-layer-shell from source..."
    local tmp; tmp=$(mktemp -d)
    git clone --depth=1 https://github.com/wmww/gtk4-layer-shell "$tmp/gtk4-layer-shell"
    pushd "$tmp/gtk4-layer-shell" >/dev/null
    meson setup build --prefix=/usr/local -Dexamples=false -Ddocs=false -Dtests=false
    ninja -C build
    sudo ninja -C build install
    sudo ldconfig 2>/dev/null || true
    popd >/dev/null
    rm -rf "$tmp"
}

# Detect package manager and install deps
if   command_exists pacman;        then install_deps_pacman
elif command_exists apt-get;       then install_deps_apt
elif command_exists dnf;           then install_deps_dnf
elif command_exists zypper;        then install_deps_zypper
elif command_exists xbps-install;  then install_deps_xbps
elif command_exists apk;           then install_deps_apk
elif command_exists emerge;        then install_deps_emerge
elif command_exists eopkg;         then install_deps_eopkg
else
    warn "No supported package manager found."
    warn "Please install manually: GTK4 dev headers, gtk4-layer-shell dev headers, grim, Rust/Cargo."
fi

# Fall back to rustup if cargo is still missing
if ! command_exists cargo; then
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source "$HOME/.cargo/env"
fi

# Validate that required libraries are present before wasting a full build
if ! pkg-config --exists gtk4 2>/dev/null; then
    error "gtk4 dev headers not found via pkg-config. Aborting."
    exit 1
fi
if ! pkg-config --exists gtk4-layer-shell 2>/dev/null; then
    error "gtk4-layer-shell dev headers not found via pkg-config. Aborting."
    exit 1
fi

# ── 2. Build ─────────────────────────────────────────────────────────────────
header "Building rust-dock (release)..."
cargo build --release

# ── 3. Install binary ────────────────────────────────────────────────────────
header "Installing binary..."
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"
install -m 755 target/release/rust-dock "$BIN_DIR/rust-dock"
info "Binary installed to $BIN_DIR/rust-dock"

# Add ~/.local/bin to PATH in shell rc files if not already present
add_to_path() {
    local rcfile="$1"
    [[ -f "$rcfile" ]] || return
    if ! grep -q 'LOCAL_BIN\|\.local/bin' "$rcfile" 2>/dev/null; then
        echo '' >> "$rcfile"
        echo '# Added by rust-dock installer' >> "$rcfile"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rcfile"
        info "Added ~/.local/bin to PATH in $rcfile"
    fi
}

add_to_path "$HOME/.bashrc"
add_to_path "$HOME/.zshrc"
add_to_path "$HOME/.profile"

# Export for the current session so the user can run it right now
export PATH="$HOME/.local/bin:$PATH"

# ── 4. Default configuration ─────────────────────────────────────────────────
header "Setting up configuration..."
CONFIG_DIR="$HOME/.config/rust-dock"
DATA_DIR="$HOME/.local/share/rust-dock"
CONFIG_FILE="$CONFIG_DIR/hypr-dock.conf"
PINNED_FILE="$DATA_DIR/pinned"

mkdir -p "$CONFIG_DIR" "$DATA_DIR"

if [ ! -f "$CONFIG_FILE" ]; then
    cat > "$CONFIG_FILE" <<'EOF'
[General]
CurrentTheme  = lotos
Position      = bottom
IconSize      = 32
Padding       = 4
Radius        = 10
Opacity       = 0.8
Exclusive     = true
SmartView     = false
AutoHideDelay = 400
SystemGapUsed = true
Margin        = 8
NoLauncher    = true
ContextPos    = 5

[General.preview]
ShowDelay  = 70
HideDelay  = 300
MoveDelay  = 30

[Theme]
Spacing = 5
EOF
    info "Created $CONFIG_FILE"
else
    info "Keeping existing $CONFIG_FILE"
fi

if [ ! -f "$PINNED_FILE" ]; then
    printf 'firefox\nkitty\ncode\n' > "$PINNED_FILE"
    info "Created $PINNED_FILE with default pinned apps"
else
    info "Keeping existing $PINNED_FILE"
fi

# ── 5. Done ───────────────────────────────────────────────────────────────────
header "Installation complete!"
echo ""
echo -e "  Run now:     ${BOLD}rust-dock${NC}"
echo -e "  Hyprland:    Add ${BOLD}exec-once = rust-dock${NC} to hyprland.conf"
echo -e "  Config:      ${BOLD}$CONFIG_FILE${NC}"
echo ""
echo "  Tip — prevent Hyprland from adding extra borders to the dock:"
echo "    windowrulev2 = noborder, class:(rust-dock)"
echo "    windowrulev2 = noborder, class:(dock-preview)"
echo ""
if ! command_exists rust-dock 2>/dev/null; then
    warn "Restart your terminal (or run: source ~/.bashrc) to use 'rust-dock' directly."
fi
