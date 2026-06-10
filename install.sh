#!/usr/bin/env bash
# rust-dock installer

export GUM_CHOOSE_CURSOR_FOREGROUND="7"
export GUM_CHOOSE_HEADER_FOREGROUND="7"
export GUM_CHOOSE_SELECTED_FOREGROUND="7"
export GUM_SPIN_SPINNER_FOREGROUND="7"
export GUM_STYLE_FOREGROUND="7"
export GUM_CONFIRM_PROMPT_FOREGROUND="7"
export GUM_CONFIRM_SELECTED_BACKGROUND="7"
export GUM_CONFIRM_SELECTED_FOREGROUND="0"

DOTFILES_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# --- UI ---

print_banner() {
    clear
    gum style \
        --foreground 4 --bold \
        "██████╗ ██╗   ██╗███████╗████████╗   ██████╗  ██████╗  ██████╗██╗  ██╗" \
        "██╔══██╗██║   ██║██╔════╝╚══██╔══╝   ██╔══██╗██╔═══██╗██╔════╝██║ ██╔╝" \
        "██████╔╝██║   ██║███████╗   ██║ █████╗██║  ██║██║   ██║██║     █████╔╝ " \
        "██╔══██╗██║   ██║╚════██║   ██║ ╚════╝██║  ██║██║   ██║██║     ██╔═██╗ " \
        "██║  ██║╚██████╔╝███████║   ██║       ██████╔╝╚██████╔╝╚██████╗██║  ██╗" \
        "╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝       ╚═════╝  ╚═════╝  ╚═════╝╚═╝  ╚═╝"
    echo ""
    gum style --foreground 8 --italic "  A Wayland dock for Hyprland — powered by Rust & GTK4"
    echo ""
}

section() {
    echo ""
    gum style --bold --margin "1 0" --underline " SECTION: $1 "
}

info()    { echo "  [SYSTEM] $1"; }
success() { echo "  [DONE]   $1"; }
warn()    { echo "  [WARN]   $1"; }

# --- CHECKS ---

if [ "$EUID" -eq 0 ]; then
    echo "Do not run as root."
    exit 1
fi

# Ensure gum is available
if ! command -v gum > /dev/null; then
    sudo pacman -S --needed --noconfirm gum
fi

print_banner
gum spin --spinner pulse --title "ACCESSING SYSTEM CORE..." -- sleep 2

# --- YAY ---

install_yay() {
    if ! command -v yay > /dev/null; then
        section "DEPENDENCY: AUR HELPER"
        gum spin --spinner dot --title "Deploying yay..." -- bash -c "
            git clone https://aur.archlinux.org/yay.git /tmp/yay-install > /dev/null 2>&1
            cd /tmp/yay-install && makepkg -si --noconfirm > /dev/null 2>&1
        "
        success "yay helper initialized."
    fi
}

# --- DEPENDENCIES ---

step_deps() {
    section "SYSTEM DEPENDENCIES"
    info "Installing build dependencies..."

    gum spin --spinner dot --title "Installing packages..." -- \
        yay -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel rust

    # Fallback: rustup if cargo still missing
    if ! command -v cargo > /dev/null; then
        gum spin --spinner dot --title "Installing Rust via rustup..." -- \
            bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path"
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi

    if ! pkg-config --exists gtk4 2>/dev/null; then
        gum style --foreground 1 "  [ERROR] gtk4 dev headers not found. Aborting."
        exit 1
    fi
    if ! pkg-config --exists gtk4-layer-shell 2>/dev/null; then
        gum style --foreground 1 "  [ERROR] gtk4-layer-shell not found. Aborting."
        exit 1
    fi

    success "Dependencies ready."
}

# --- BUILD ---

step_build() {
    section "BUILD"
    gum spin --spinner moon --title "Compiling rust-dock (release)..." -- \
        cargo build --release
    success "Build complete."
}

# --- INSTALL BINARY ---

step_install_bin() {
    section "BINARY INSTALLATION"
    BIN_DIR="$HOME/.local/bin"
    mkdir -p "$BIN_DIR"
    install -m 755 "$DOTFILES_DIR/target/release/rust-dock" "$BIN_DIR/rust-dock"
    info "Binary installed to $BIN_DIR/rust-dock"

    add_to_path() {
        local rcfile="$1"
        [[ -f "$rcfile" ]] || return
        if ! grep -q '\.local/bin' "$rcfile" 2>/dev/null; then
            echo '' >> "$rcfile"
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rcfile"
            info "Added ~/.local/bin to PATH in $rcfile"
        fi
    }
    add_to_path "$HOME/.bashrc"
    add_to_path "$HOME/.zshrc"
    add_to_path "$HOME/.profile"
    export PATH="$BIN_DIR:$PATH"

    success "Binary ready."
}

# --- CONFIG ---

step_config() {
    section "CONFIGURATION"
    CONFIG_DIR="$HOME/.config/rust-dock"
    DATA_DIR="$HOME/.local/share/rust-dock"
    CONFIG_FILE="$CONFIG_DIR/hypr-dock.conf"
    PINNED_FILE="$DATA_DIR/pinned"

    mkdir -p "$CONFIG_DIR" "$DATA_DIR"

    if [ ! -f "$CONFIG_FILE" ]; then
        cat > "$CONFIG_FILE" <<'EOF'
[General]
CurrentTheme  = default
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
        # Solo agrega apps que existen en el sistema
        > "$PINNED_FILE"
        for app in firefox chromium kitty foot alacritty code codium thunar nautilus; do
            if command -v "$app" > /dev/null 2>&1 || \
               find /usr/share/applications /home/"$USER"/.local/share/applications \
                    -name "${app}.desktop" 2>/dev/null | grep -q .; then
                echo "$app" >> "$PINNED_FILE"
            fi
        done
        # Si no encontró nada, deja el archivo vacío (la dock arranca igual)
        info "Created $PINNED_FILE"
    else
        info "Keeping existing $PINNED_FILE"
    fi

    success "Configuration ready."
}

# --- HYPRLAND INTEGRATION ---

step_hyprland() {
    section "HYPRLAND INTEGRATION"
    HYPR_CONF="$HOME/.config/hypr/hyprland.conf"

    if [ ! -f "$HYPR_CONF" ]; then
        warn "hyprland.conf not found — skipping auto-config."
        return
    fi

    if ! grep -q "rust-dock" "$HYPR_CONF" 2>/dev/null; then
        if gum confirm "Add 'exec-once = rust-dock' to hyprland.conf?"; then
            echo "" >> "$HYPR_CONF"
            echo "exec-once = rust-dock" >> "$HYPR_CONF"
            info "Added exec-once = rust-dock to hyprland.conf"
        fi
    else
        info "rust-dock already present in hyprland.conf"
    fi

    success "Hyprland integration ready."
}

# --- PYWAL TEMPLATE ---

step_pywal() {
    section "PYWAL INTEGRATION"
    WAL_TEMPLATES="$HOME/.config/wal/templates"

    if ! command -v wal > /dev/null 2>&1; then
        warn "pywal not installed — skipping template setup."
        return
    fi

    mkdir -p "$WAL_TEMPLATES"

    if [ ! -f "$WAL_TEMPLATES/colors-waybar.css" ]; then
        cat > "$WAL_TEMPLATES/colors-waybar.css" <<'EOF'
/*
 * Waybar / rust-dock color theme — generated by pywal
 */

@define-color background {background};
@define-color foreground {foreground};
@define-color cursor {cursor};

@define-color color0  {color0};
@define-color color1  {color1};
@define-color color2  {color2};
@define-color color3  {color3};
@define-color color4  {color4};
@define-color color5  {color5};
@define-color color6  {color6};
@define-color color7  {color7};
@define-color color8  {color8};
@define-color color9  {color9};
@define-color color10 {color10};
@define-color color11 {color11};
@define-color color12 {color12};
@define-color color13 {color13};
@define-color color14 {color14};
@define-color color15 {color15};
EOF
        info "Created pywal template for rust-dock"
    else
        info "Keeping existing pywal template"
    fi

    # Symlink para waybar si no existe
    WAYBAR_COLORS="$HOME/.config/waybar/colors-pywal.css"
    WAL_CACHE="$HOME/.cache/wal/colors-waybar.css"
    if [ ! -e "$WAYBAR_COLORS" ] && [ -d "$HOME/.config/waybar" ]; then
        ln -sf "$WAL_CACHE" "$WAYBAR_COLORS"
        info "Linked colors-pywal.css → ~/.cache/wal/colors-waybar.css"
    fi

    success "Pywal integration ready."
}

# --- EXECUTION ---

install_yay
step_deps
step_build
step_install_bin
step_config
step_hyprland
step_pywal

section "DEPLOYMENT COMPLETE"
echo ""
echo "  Run now:  rust-dock"
echo "  Config:   ~/.config/rust-dock/hypr-dock.conf"
echo "  Pinned:   ~/.local/share/rust-dock/pinned"
echo ""
success "rust-dock deployed."
