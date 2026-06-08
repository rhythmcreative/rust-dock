use gtk4::CssProvider;
use crate::config::Config;

pub fn load_css(config: &Config) {
    let provider = CssProvider::new();
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
            transition: background-color 120ms ease;
        }}
        button:hover {{
            background-color: alpha(@color1, 0.28);
        }}
        .launcher-btn {{ color: @color2; }}
        .running {{ border-bottom: 2px solid @color4; }}
        .indicator {{
            font-size: 8px;
            color: @color1;
            margin-top: -2px;
        }}

        /* ── Preview window (dock-preview layer surface) ──── */
        /* Fully transparent: only the win-card boxes are visible */
        .dock-preview-window {{
            background-color: transparent;
            border: none;
            border-radius: 0;
            box-shadow: none;
        }}
        .preview-row {{
            background-color: transparent;
            /* Large transparent padding acts as an invisible bridge to the dock */
            padding: 20px;
        }}

        /* ── Window preview cards ──────────────────────────── */
        .win-card {{
            background-color: alpha(@background, 0.88);
            border-radius: 12px;
            border: 1px solid alpha(@color1, 0.25);
            min-width: 172px;
            transition: background-color 150ms ease, box-shadow 150ms ease;
            box-shadow: 0 8px 24px alpha(#000, 0.45);
        }}
        .win-card:hover {{
            background-color: alpha(@color1, 0.18);
            border-color: alpha(@color1, 0.5);
            box-shadow: 0 10px 30px alpha(#000, 0.55);
        }}
        .win-title {{
            font-size: 11px;
            font-weight: 600;
            color: @foreground;
        }}
        .win-close-btn {{
            background-color: transparent;
            border: none;
            border-radius: 50%;
            padding: 1px 4px;
            color: alpha(@foreground, 0.45);
            font-size: 14px;
            min-height: 0;
            min-width: 0;
            transition: background-color 100ms ease, color 100ms ease;
        }}
        .win-close-btn:hover {{
            background-color: #e74c3c;
            color: white;
        }}
        .win-thumb-box {{
            background-color: alpha(@color0, 0.4);
            border-radius: 0 0 10px 10px;
            margin: 0 4px 4px 4px;
            min-height: 88px;
        }}
        .win-thumbnail {{
            border-radius: 0 0 8px 8px;
        }}
        .preview-placeholder {{
            opacity: 0.35;
        }}

        /* ── Right-click context menu ─────────────────────── */
        .context-popover > contents {{
            background-color: alpha(@background, 0.97);
            border-radius: 10px;
            border: 1px solid alpha(@color1, 0.2);
            padding: 0;
        }}
        .pop-action-btn {{
            background-color: alpha(@color1, 0.1);
            border-radius: 7px;
            padding: 5px 12px;
            font-size: 12px;
            color: @foreground;
            border: 1px solid alpha(@color1, 0.18);
            transition: background-color 120ms ease;
        }}
        .pop-action-btn:hover {{
            background-color: alpha(@color1, 0.26);
        }}
        .pop-close-btn {{
            color: alpha(@foreground, 0.85);
        }}
        .pop-close-btn:hover {{
            background-color: alpha(#e74c3c, 0.5);
            color: white;
            border-color: alpha(#e74c3c, 0.6);
        }}
    ",
    opacity = config.opacity,
    radius = config.radius,
    padding = config.padding,
    btn_padding = config.padding / 2,
    btn_radius = config.radius - 2
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

    provider.load_from_data(&css_data);

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
