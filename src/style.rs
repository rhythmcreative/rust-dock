use gtk4::CssProvider;
use crate::config::Config;
use std::cell::RefCell;

thread_local! {
    /// A single CssProvider reused across reloads. Previously every call created
    /// and registered a new provider without removing the old ones, leaking
    /// providers on every Hyprland event and slowing style recalculation.
    static PROVIDER: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
}

pub fn load_css(config: &Config) {
    let mut css_data = String::new();

    if let Some(mut pywal_path) = dirs::cache_dir() {
        pywal_path.push("wal/colors-waybar.css");
        if pywal_path.exists() {
            if let Ok(pywal_css) = std::fs::read_to_string(pywal_path) {
                css_data.push_str(&pywal_css);
            }
        }
    }

    let default_css = format!("
        /* ── Dock window (only rust-dock, not dock-preview) ─── */
        window:not(.dock-preview-window) {{
            background-color: alpha(@background, {opacity});
            border-radius: {radius}px;
            border: 1px solid alpha(@color1, 0.35);
        }}
        .dock-container {{
            padding: {padding}px;
        }}
        button {{
            background-color: transparent;
            border: none;
            padding: {btn_padding}px;
            margin: 2px;
            border-radius: {btn_radius}px;
            color: @foreground;
            min-height: 0;
            min-width: 0;
            transition: background-color 140ms ease, border-color 140ms ease;
        }}
        button:hover {{
            background-color: alpha(@color1, 0.28);
        }}
        .launcher-btn {{ color: @color2; }}
        .running {{ border-bottom: 2px solid @color4; }}
        /* Highlight the icon of the currently focused app. */
        button.active {{
            background-color: alpha(@color1, 0.22);
            border-bottom: 2px solid @color2;
        }}
        .indicator {{
            font-size: 8px;
            color: @color1;
            margin-top: -2px;
        }}

        /* ── Preview window (dock-preview layer surface) ──── */
        window.dock-preview-window {{
            background: transparent !important;
            border: none !important;
            box-shadow: none !important;
        }}
        /* The preview-row is the direct child of the preview window, so we
           style it as a single cohesive panel that groups all window cards. */
        .preview-row {{
            background-color: alpha(@background, 0.92);
            border: none;
            border-radius: 12px;
            padding: 8px;
            box-shadow: 0 6px 24px alpha(#000, 0.35);
        }}
        /* ── Window preview cards ── */
        /* Each card is its own separated, rounded tile. */
        .win-card {{
            background-color: alpha(@color1, 0.08);
            border: 1px solid alpha(@color1, 0.18);
            border-radius: 10px;
            padding: 6px;
            min-width: 200px;
            transition: background-color 120ms ease, border-color 120ms ease;
        }}
        .win-card:hover {{
            background-color: alpha(@color1, 0.16);
            border-color: alpha(@color1, 0.45);
        }}
        .win-thumb-box {{
            background-color: alpha(@background, 0.6);
            border: none;
            border-radius: 10px;
            min-height: 120px;
            min-width: 200px;
        }}
        .win-title {{
            font-size: 11px;
            font-weight: 500;
            color: alpha(@foreground, 0.85);
        }}
        .win-close-btn {{
            background-color: transparent;
            border: none;
            border-radius: 50%;
            color: alpha(@foreground, 0.5);
            font-size: 12px;
            font-weight: bold;
            min-height: 18px;
            min-width: 18px;
            padding: 0;
            transition: background-color 120ms ease;
        }}
        .win-close-btn:hover {{
            background-color: #e74c3c;
            color: white;
        }}
        .win-thumbnail {{
            border-radius: 10px;
        }}
        .preview-placeholder {{
            opacity: 0.5;
        }}

        /* ── Right-click context menu ─────────────────────── */
        /* Kill the default popover chrome AND the heavy shadow: on a transparent
           layer-shell surface a wide box-shadow paints as a flat dark slab on
           top. Only the rounded `> contents` panel should be visible. */
        popover.context-popover,
        popover.context-popover > arrow,
        popover.context-popover > contents {{
            background: none;
            background-color: transparent;
            border: none;
            box-shadow: none;
            margin: 0;
            padding: 0;
        }}
        popover.context-popover > arrow {{
            min-height: 0;
            min-width: 0;
        }}
        popover.context-popover > contents {{
            background-color: @background;
            border-radius: 12px;
            border: 1px solid alpha(@color1, 0.30);
            padding: 6px;
        }}
        .ctx-header {{
            padding: 6px 12px 8px 12px;
        }}
        .ctx-header-label {{
            font-size: 13px;
            font-weight: 700;
            color: @foreground;
        }}
        .ctx-menu-icon {{
            color: alpha(@foreground, 0.7);
        }}
        .ctx-menu-item {{
            background-color: transparent;
            border: none;
            border-radius: 8px;
            padding: 8px 12px;
            margin: 1px 0;
            font-size: 12px;
            font-weight: 500;
            color: @foreground;
            min-height: 0;
            min-width: 180px;
            transition: background-color 120ms ease;
        }}
        .ctx-menu-item:hover {{
            background-color: alpha(@color1, 0.22);
        }}
        .ctx-close-item {{
            color: alpha(#e74c3c, 0.9);
        }}
        .ctx-close-item .ctx-menu-icon {{
            color: alpha(#e74c3c, 0.9);
        }}
        .ctx-close-item:hover {{
            background-color: alpha(#e74c3c, 0.16);
            color: #e74c3c;
        }}
        .ctx-close-item:hover .ctx-menu-icon {{
            color: #e74c3c;
        }}
        .context-popover separator {{
            background-color: alpha(@color1, 0.18);
            margin: 5px 8px;
            min-height: 1px;
        }}

        /* ── Workspace switcher ────────────────────────────── */
        .dock-ws-separator {{
            background-color: alpha(@color1, 0.20);
            margin: 4px 2px;
            min-width: 1px;
            min-height: 1px;
        }}
        .workspace-bar {{
            padding: 0;
        }}
        .ws-btn {{
            background-color: transparent;
            border: none;
            border-radius: {ws_radius}px;
            min-width: 22px;
            min-height: 22px;
            padding: 2px 5px;
            margin: 1px;
            transition: background-color 120ms ease, border-color 120ms ease;
        }}
        .ws-btn:hover {{
            background-color: alpha(@color1, 0.22);
        }}
        .ws-btn.ws-active {{
            background-color: alpha(@color2, 0.30);
            border: 1px solid alpha(@color2, 0.60);
        }}
        .ws-btn.ws-occupied:not(.ws-active) {{
            border: 1px solid alpha(@color1, 0.30);
        }}
        .ws-label {{
            font-size: 11px;
            font-weight: 600;
            color: alpha(@foreground, 0.75);
            min-width: 10px;
        }}
        .ws-btn.ws-active .ws-label {{
            color: @color2;
        }}
    ",
    opacity = config.opacity,
    radius = config.radius,
    padding = config.padding,
    btn_padding = config.padding / 2,
    btn_radius = config.radius - 2,
    ws_radius = (config.radius - 2).max(4)
    );

    css_data.push_str(&default_css);

    if let Some(path) = &config.style {
        if path.exists() {
            if let Ok(user_css) = std::fs::read_to_string(path) {
                css_data.push_str(&user_css);
            }
        }
    }

    if let Some(mut theme_path) = dirs::config_dir() {
        theme_path.push(format!("rust-dock/themes/{}/style.css", config.current_theme));
        if theme_path.exists() {
            if let Ok(theme_css) = std::fs::read_to_string(theme_path) {
                css_data.push_str(&theme_css);
            }
        }
    }

    PROVIDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provider = slot.get_or_insert_with(|| {
            let prov = CssProvider::new();
            if let Some(display) = gtk4::gdk::Display::default() {
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &prov,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            prov
        });
        provider.load_from_data(&css_data);
    });
}
