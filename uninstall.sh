#!/usr/bin/env bash
# rust-dock uninstaller

set -euo pipefail

export GUM_CONFIRM_PROMPT_FOREGROUND="7"
export GUM_CONFIRM_SELECTED_BACKGROUND="7"
export GUM_CONFIRM_SELECTED_FOREGROUND="0"
export GUM_STYLE_FOREGROUND="7"

# ── gum requerido ─────────────────────────────────────────────────────────────
if ! command -v gum > /dev/null; then
    echo "  [ERROR] gum not found. Install it first or run: sudo pacman -S gum"
    exit 1
fi

# ── UI ────────────────────────────────────────────────────────────────────────

print_banner() {
    clear
    gum style \
        --foreground 1 --bold \
        "██████╗ ██╗   ██╗███████╗████████╗     ██████╗  ██████╗  ██████╗██╗  ██╗" \
        "██╔══██╗██║   ██║██╔════╝╚══██╔══╝     ██╔══██╗██╔═══██╗██╔════╝██║ ██╔╝" \
        "██████╔╝██║   ██║███████╗   ██║ █████╗ ██║  ██║██║   ██║██║     █████╔╝ " \
        "██╔══██╗██║   ██║╚════██║   ██║ ╚════╝ ██║  ██║██║   ██║██║     ██╔═██╗ " \
        "██║  ██║╚██████╔╝███████║   ██║        ██████╔╝╚██████╔╝╚██████╗██║  ██╗" \
        "╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝        ╚═════╝  ╚═════╝  ╚═════╝╚═╝  ╚═╝"
    echo ""
    gum style --foreground 8 --italic "  Uninstaller"
    echo ""
}

section() {
    echo ""
    gum style --bold --margin "1 0" --underline " SECTION: $1 "
}

info()    { echo "  [SYSTEM] $1"; }
success() { echo "  [DONE]   $1"; }

# ── Checks ────────────────────────────────────────────────────────────────────

if [ "$EUID" -eq 0 ]; then
    echo "Do not run as root."
    exit 1
fi

print_banner

gum style --foreground 3 "  This will remove rust-dock from your system."
echo ""

if ! gum confirm "Continue with uninstallation?"; then
    echo "  Aborted."
    exit 0
fi

# ── Detener instancias ────────────────────────────────────────────────────────

section "STOPPING RUST-DOCK"
if pgrep -x rust-dock > /dev/null; then
    pkill -x rust-dock || true
    info "Stopped running instance."
else
    info "No running instance found."
fi
success "Done."

# ── Binario ───────────────────────────────────────────────────────────────────

section "BINARY"
BIN="$HOME/.local/bin/rust-dock"
if [ -f "$BIN" ]; then
    rm -f "$BIN"
    info "Removed $BIN"
else
    info "Binary not found — skipping."
fi
success "Done."

# ── Configuración y datos ─────────────────────────────────────────────────────

section "CONFIGURATION & DATA"
CONFIG_DIR="$HOME/.config/rust-dock"
DATA_DIR="$HOME/.local/share/rust-dock"

if [ -d "$CONFIG_DIR" ] || [ -d "$DATA_DIR" ]; then
    if gum confirm "Remove config and pinned apps? ($CONFIG_DIR, $DATA_DIR)"; then
        rm -rf "$CONFIG_DIR" "$DATA_DIR"
        info "Removed config and data."
    else
        info "Keeping config and data."
    fi
else
    info "No config or data found — skipping."
fi
success "Done."

# ── Hyprland autostart ────────────────────────────────────────────────────────

section "HYPRLAND INTEGRATION"
HYPR_CONF="$HOME/.config/hypr/hyprland.conf"
if [ -f "$HYPR_CONF" ] && grep -q "rust-dock" "$HYPR_CONF" 2>/dev/null; then
    if gum confirm "Remove 'exec-once = rust-dock' from hyprland.conf?"; then
        sed -i '/exec-once\s*=\s*rust-dock/d' "$HYPR_CONF"
        # Eliminar línea en blanco que pudo quedar sola al final del bloque
        sed -i '/^[[:space:]]*$/N;/^\n[[:space:]]*$/d' "$HYPR_CONF"
        info "Removed from hyprland.conf"
    else
        info "Keeping hyprland.conf entry."
    fi
else
    info "No rust-dock entry in hyprland.conf — skipping."
fi
success "Done."

# ── Pywal template ────────────────────────────────────────────────────────────

section "PYWAL TEMPLATE"
WAL_TEMPLATE="$HOME/.config/wal/templates/colors-waybar.css"
if [ -f "$WAL_TEMPLATE" ]; then
    if gum confirm "Remove pywal template ($WAL_TEMPLATE)?"; then
        rm -f "$WAL_TEMPLATE"
        info "Removed pywal template."
    else
        info "Keeping pywal template."
    fi
else
    info "Pywal template not found — skipping."
fi
success "Done."

# ── Symlink waybar ────────────────────────────────────────────────────────────

section "WAYBAR SYMLINK"
WAYBAR_COLORS="$HOME/.config/waybar/colors-pywal.css"
if [ -L "$WAYBAR_COLORS" ]; then
    if gum confirm "Remove waybar colors symlink ($WAYBAR_COLORS)?"; then
        rm -f "$WAYBAR_COLORS"
        info "Removed symlink."
    else
        info "Keeping symlink."
    fi
else
    info "Waybar symlink not found — skipping."
fi
success "Done."

# ── Archivos temporales ───────────────────────────────────────────────────────

section "TEMP FILES"
rm -f /tmp/rust-dock-preview-*.png 2>/dev/null || true
info "Cleaned /tmp/rust-dock-preview-*.png"
success "Done."

# ── Fin ───────────────────────────────────────────────────────────────────────

section "UNINSTALL COMPLETE"
echo ""
gum style --foreground 8 --italic "  rust-dock has been removed from your system."
echo ""
