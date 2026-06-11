#!/usr/bin/env bash
# rust-dock installer — multi-distro

set -euo pipefail

export GUM_CHOOSE_CURSOR_FOREGROUND="7"
export GUM_CHOOSE_HEADER_FOREGROUND="7"
export GUM_CHOOSE_SELECTED_FOREGROUND="7"
export GUM_SPIN_SPINNER_FOREGROUND="7"
export GUM_STYLE_FOREGROUND="7"
export GUM_CONFIRM_PROMPT_FOREGROUND="7"
export GUM_CONFIRM_SELECTED_BACKGROUND="7"
export GUM_CONFIRM_SELECTED_FOREGROUND="0"

DOTFILES_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# ── Bootstrap: instalar gum antes del banner ──────────────────────────────────
# gum es el único requisito previo — se instala por pkg manager o binario GitHub.
ensure_gum() {
    command -v gum > /dev/null && return 0

    echo "  [BOOT] Installing gum..."

    if   command -v pacman    > /dev/null; then sudo pacman -S --needed --noconfirm gum
    elif command -v apt-get   > /dev/null; then
        # Charm repo oficial para Debian/Ubuntu
        mkdir -p /etc/apt/keyrings
        curl -fsSL https://repo.charm.sh/apt/gpg.key \
            | sudo gpg --dearmor -o /etc/apt/keyrings/charm.gpg
        echo "deb [signed-by=/etc/apt/keyrings/charm.gpg] https://repo.charm.sh/apt/ * *" \
            | sudo tee /etc/apt/sources.list.d/charm.list > /dev/null
        sudo apt-get update -qq && sudo apt-get install -y gum
    elif command -v dnf        > /dev/null; then
        echo '[charm]
name=Charm
baseurl=https://repo.charm.sh/yum/
enabled=1
gpgcheck=1
gpgkey=https://repo.charm.sh/yum/gpg.key' | sudo tee /etc/yum.repos.d/charm.repo > /dev/null
        sudo dnf install -y gum
    elif command -v zypper     > /dev/null; then sudo zypper install -y gum
    elif command -v xbps-install > /dev/null; then sudo xbps-install -Sy gum
    elif command -v apk        > /dev/null; then sudo apk add --no-cache gum
    else
        # Fallback: descargar binario de GitHub Releases
        local ver
        ver=$(curl -fsSL https://api.github.com/repos/charmbracelet/gum/releases/latest \
              | grep '"tag_name"' | cut -d'"' -f4 | tr -d 'v')
        local arch
        arch=$(uname -m | sed 's/x86_64/x86_64/;s/aarch64/arm64/')
        curl -fsSL \
            "https://github.com/charmbracelet/gum/releases/download/v${ver}/gum_${ver}_linux_${arch}.tar.gz" \
            | tar -xz -C /tmp gum
        sudo install -m 755 /tmp/gum /usr/local/bin/gum
    fi
}

if [ "$EUID" -eq 0 ]; then
    echo "Do not run as root. The installer uses sudo internally where needed."
    exit 1
fi

ensure_gum

# ── UI ────────────────────────────────────────────────────────────────────────

print_banner() {
    clear
    gum style \
        --foreground 4 --bold \
        "██████╗ ██╗   ██╗███████╗████████╗     ██████╗  ██████╗  ██████╗██╗  ██╗" \
        "██╔══██╗██║   ██║██╔════╝╚══██╔══╝     ██╔══██╗██╔═══██╗██╔════╝██║ ██╔╝" \
        "██████╔╝██║   ██║███████╗   ██║ █████╗ ██║  ██║██║   ██║██║     █████╔╝ " \
        "██╔══██╗██║   ██║╚════██║   ██║ ╚════╝ ██║  ██║██║   ██║██║     ██╔═██╗ " \
        "██║  ██║╚██████╔╝███████║   ██║        ██████╔╝╚██████╔╝╚██████╗██║  ██╗" \
        "╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝        ╚═════╝  ╚═════╝  ╚═════╝╚═╝  ╚═╝"
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

# ── Checks ────────────────────────────────────────────────────────────────────

print_banner
gum spin --spinner pulse --title "ACCESSING SYSTEM CORE..." -- sleep 2

# ── Detección de distro ───────────────────────────────────────────────────────

detect_pm() {
    if   command -v pacman      > /dev/null; then echo "pacman"
    elif command -v apt-get     > /dev/null; then echo "apt"
    elif command -v dnf         > /dev/null; then echo "dnf"
    elif command -v zypper      > /dev/null; then echo "zypper"
    elif command -v xbps-install > /dev/null; then echo "xbps"
    elif command -v apk         > /dev/null; then echo "apk"
    elif command -v emerge      > /dev/null; then echo "emerge"
    elif command -v eopkg       > /dev/null; then echo "eopkg"
    else echo "unknown"
    fi
}

PM=$(detect_pm)

# ── AUR helper (solo Arch) ────────────────────────────────────────────────────

install_yay() {
    [[ "$PM" != "pacman" ]] && return 0
    command -v yay > /dev/null && return 0

    section "AUR HELPER"
    gum spin --spinner dot --title "Deploying yay..." -- bash -c "
        git clone https://aur.archlinux.org/yay.git /tmp/yay-install > /dev/null 2>&1
        cd /tmp/yay-install && makepkg -si --noconfirm > /dev/null 2>&1
    "
    success "yay initialized."
}

# ── gtk4-layer-shell desde source (distros sin paquete) ──────────────────────

install_layer_shell_from_source() {
    info "Building gtk4-layer-shell from source..."
    local tmp; tmp=$(mktemp -d)
    git clone --depth=1 https://github.com/wmww/gtk4-layer-shell "$tmp/gtk4-layer-shell" > /dev/null 2>&1
    pushd "$tmp/gtk4-layer-shell" > /dev/null
    meson setup build --prefix=/usr/local -Dexamples=false -Ddocs=false -Dtests=false > /dev/null 2>&1
    ninja -C build > /dev/null 2>&1
    sudo ninja -C build install > /dev/null 2>&1
    sudo ldconfig 2>/dev/null || true
    popd > /dev/null
    rm -rf "$tmp"
}

# ── Dependencias por distro ───────────────────────────────────────────────────

step_deps() {
    section "SYSTEM DEPENDENCIES"
    info "Installing packages via $PM..."

    # Nota: NO usar gum spin aquí — los gestores de paquetes necesitan
    # stdin para cosas como importar claves GPG de AUR o pedir sudo.
    case "$PM" in
    pacman)
        local pm=pacman
        command -v yay  > /dev/null && pm=yay
        command -v paru > /dev/null && pm=paru
        # No instalar rust/cargo si ya está disponible (ej: vía rustup)
        local rust_pkg=""
        command -v cargo > /dev/null || rust_pkg="rust"
        "$pm" -S --needed --noconfirm gtk4 gtk4-layer-shell grim pkgconf base-devel $rust_pkg
        ;;
    apt)
        sudo apt-get update -qq
        local layer_pkg=libgtk4-layer-shell-dev
        apt-cache show "$layer_pkg" > /dev/null 2>&1 || layer_pkg=libgtk4-layer-shell1-dev
        sudo apt-get install -y libgtk-4-dev "$layer_pkg" grim pkg-config \
            build-essential libglib2.0-dev libcairo2-dev cargo rustc
        ;;
    dnf)
        sudo dnf install -y gtk4-devel gtk4-layer-shell-devel grim \
            pkgconf-pkg-config gcc gcc-c++ rust cargo
        ;;
    zypper)
        sudo zypper install -y gtk4-devel gtk4-layer-shell-devel grim \
            pkg-config gcc gcc-c++ rust cargo
        ;;
    xbps)
        sudo xbps-install -Sy gtk4-devel gtk4-layer-shell-devel grim \
            pkg-config gcc rust cargo
        ;;
    apk)
        sudo apk add --no-cache gtk4.0-dev grim pkgconf build-base rust cargo
        ;;
    emerge)
        sudo emerge --noreplace x11-libs/gtk+ dev-libs/gtk4-layer-shell \
            x11-apps/grim dev-util/pkgconf virtual/rust
        ;;
    eopkg)
        sudo eopkg install -y libgtk-4-devel grim pkg-config gcc rust cargo
        ;;
    *)
        gum style --foreground 1 "  [ERROR] Unknown package manager. Install dependencies manually."
        exit 1
        ;;
    esac

    # gtk4-layer-shell desde source si el pkg manager no la tiene (Alpine, Solus, etc.)
    if ! pkg-config --exists gtk4-layer-shell 2>/dev/null; then
        info "gtk4-layer-shell not found via pkg-config — building from source..."
        install_layer_shell_from_source
    fi

    # Rust via rustup si cargo sigue sin estar
    if ! command -v cargo > /dev/null; then
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
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

# ── Build ─────────────────────────────────────────────────────────────────────

step_build() {
    section "BUILD"
    info "Compiling rust-dock (release) — this may take a minute..."
    if ! cargo build --release; then
        gum style --foreground 1 "  [ERROR] Build failed. See output above."
        exit 1
    fi
    success "Build complete."
}

# ── Instalar binario ──────────────────────────────────────────────────────────

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

# ── Configuración ─────────────────────────────────────────────────────────────

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
        > "$PINNED_FILE"
        for app in firefox chromium kitty foot alacritty code codium thunar nautilus; do
            if command -v "$app" > /dev/null 2>&1 || \
               find /usr/share/applications "$HOME/.local/share/applications" \
                    -name "${app}.desktop" 2>/dev/null | grep -q .; then
                echo "$app" >> "$PINNED_FILE"
            fi
        done
        info "Created $PINNED_FILE"
    else
        info "Keeping existing $PINNED_FILE"
    fi

    success "Configuration ready."
}

# ── Hyprland ──────────────────────────────────────────────────────────────────

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
            info "Added exec-once = rust-dock"
        fi
    else
        info "rust-dock already present in hyprland.conf"
    fi

    # SUPER+SPACE toggle keybind (sends SIGUSR1 to the running dock).
    if ! grep -qE 'SIGUSR1.*rust-dock|rust-dock.*SIGUSR1' "$HYPR_CONF" 2>/dev/null; then
        if gum confirm "Add SUPER+SPACE keybind to toggle rust-dock? (check for conflicts first)"; then
            echo "" >> "$HYPR_CONF"
            echo "bind = SUPER, SPACE, exec, rust-dock --toggle" >> "$HYPR_CONF"
            info "Added SUPER+SPACE toggle keybind"
        fi
    else
        info "rust-dock toggle keybind already in hyprland.conf"
    fi

    success "Hyprland integration ready."
}

# ── Pywal ─────────────────────────────────────────────────────────────────────

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
@define-color cursor     {cursor};

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
        info "Created pywal template"
    else
        info "Keeping existing pywal template"
    fi

    WAYBAR_COLORS="$HOME/.config/waybar/colors-pywal.css"
    WAL_CACHE="$HOME/.cache/wal/colors-waybar.css"
    if [ ! -e "$WAYBAR_COLORS" ] && [ -d "$HOME/.config/waybar" ]; then
        ln -sf "$WAL_CACHE" "$WAYBAR_COLORS"
        info "Linked colors-pywal.css → $WAL_CACHE"
    fi

    success "Pywal integration ready."
}

# ── Ejecución ─────────────────────────────────────────────────────────────────

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
