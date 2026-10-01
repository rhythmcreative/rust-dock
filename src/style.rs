use gtk4::CssProvider;
use crate::config::Config;
use std::cell::RefCell;
use std::time::SystemTime;

thread_local! {
    static PROVIDER: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
    static SURFACE_PROVIDER: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
    /// mtime of colors-waybar.css at last CSS load — used for polling fallback.
    static LAST_PYWAL_MTIME: RefCell<Option<SystemTime>> = const { RefCell::new(None) };
}

/// Returns true (and updates the stored mtime) if the pywal colors file
/// has been modified since the last call. Used as a fallback when the
/// inotify watcher misses events (atomic rename, directory recreation, etc.).
#[allow(dead_code)]
pub fn pywal_file_changed() -> bool {
    let path = match dirs::cache_dir() {
        Some(mut p) => { p.push("wal/colors-waybar.css"); p }
        None => return false,
    };
    let mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    LAST_PYWAL_MTIME.with(|last| {
        let mut last = last.borrow_mut();
        if last.as_ref() != Some(&mtime) {
            *last = Some(mtime);
            true
        } else {
            false
        }
    })
}

pub fn load_css(config: &Config) {
    let mut css_data = String::new();

    // Fallback palette — used when pywal has never run.
    // pywal's @define-color block (appended next) overrides these.
    css_data.push_str(
        "@define-color background #1e1e2e;\n\
         @define-color foreground #cdd6f4;\n\
         @define-color cursor     #f5e0dc;\n\
         @define-color color0  #45475a;\n\
         @define-color color1  #f38ba8;\n\
         @define-color color2  #a6e3a1;\n\
         @define-color color3  #f9e2af;\n\
         @define-color color4  #89b4fa;\n\
         @define-color color5  #f5c2e7;\n\
         @define-color color6  #94e2d5;\n\
         @define-color color7  #bac2de;\n\
         @define-color color8  #585b70;\n\
         @define-color color9  #f38ba8;\n\
         @define-color color10 #a6e3a1;\n\
         @define-color color11 #f9e2af;\n\
         @define-color color12 #89b4fa;\n\
         @define-color color13 #f5c2e7;\n\
         @define-color color14 #94e2d5;\n\
         @define-color color15 #a6adc8;\n",
    );

    if let Some(mut pywal_path) = dirs::cache_dir() {
        pywal_path.push("wal/colors-waybar.css");
        if pywal_path.exists()
            && let Ok(pywal_css) = std::fs::read_to_string(pywal_path) {
                css_data.push_str(&pywal_css);
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
            transition: transform 220ms cubic-bezier(0.4, 0, 1, 1), opacity 200ms ease;
        }}
        .dock-container.dock-sliding {{
            transform: {slide_out_transform};
            opacity: 0;
            transition: transform 220ms cubic-bezier(0.4, 0, 1, 1), opacity 220ms cubic-bezier(0.4, 0, 1, 1);
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
        button.dragging {{
            opacity: 0.45;
        }}
        button.drop-target {{
            background-color: alpha(@color2, 0.28);
            border: 1px solid alpha(@color2, 0.70);
            border-radius: {btn_radius}px;
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
        button.app-minimized {{
            opacity: 0.5;
        }}
        button.app-minimized .indicator {{
            color: @color8;
        }}

        /* ── Preview window (dock-preview layer surface) ──── */
        /* GTK4 adds a `decoration` node for CSD shadows even when set_decorated(false).
           Must be zeroed out or a visible border/shadow appears around the window. */
        window.dock-preview-window,
        window.dock-preview-window > contents,
        window.dock-preview-window decoration,
        window.dock-preview-window decoration:backdrop {{
            background: transparent;
            background-color: transparent;
            border: none;
            box-shadow: none;
            margin: 0;
            padding: 0;
        }}
        /* Round the preview surface itself: the global gtk.css paints this
           window with the panel colour and nothing else gives it a radius, so
           the preview outline came out square. */
        window.dock-preview-window,
        window.dock-preview-window.background {{
            border-radius: {preview_radius}px;
        }}
        /* Same fix for the main dock window decoration node. */
        window:not(.dock-preview-window) decoration {{
            box-shadow: none;
            border: none;
            margin: 0;
        }}
        /* ── Preview panel ────────────────────────────────────────────────── */
        .preview-row {{
            /* Opaque on purpose: the layer-shell surface behind the panel is
               painted by the global gtk.css and is not rounded, so a
               translucent panel showed that square through its own corners. */
            background-color: @background;
            border-radius: {preview_radius}px;
            border: none;
            padding: 10px;
        }}
        /* The panel floats above the dock, so every corner is rounded. The old
           dock-side variants flattened two corners and made the preview read as
           a square slab. */
        .preview-row-bottom,
        .preview-row-top,
        .preview-row-left,
        .preview-row-right {{ border: none; border-radius: {preview_radius}px; }}

        /* ── Window preview cards (Windows taskbar style) ─────────────────── */
        /* No borders and no box-shadow: on a transparent layer-shell surface a
           shadow paints as a flat square slab that ignores border-radius. */
        .win-card {{
            border-radius: {card_radius}px;
            border: none;
            background-color: alpha(@background, 0.96);
            min-width: {card_min_w}px;
            transition: background-color 140ms ease;
        }}
        .win-card:hover {{
            background-color: alpha(@background, 1.0);
        }}
        /* Header bar: icon + title + close button. */
        .win-header {{
            background-color: alpha(@background, 0.75);
            border: none;
            border-top-left-radius: {card_radius}px;
            border-top-right-radius: {card_radius}px;
            padding: 5px 6px 5px 16px;
        }}
        .win-title {{
            font-size: 11px;
            font-weight: 500;
            color: @foreground;
        }}
        /* Close button: visible, turns red on hover. */
        .win-close-btn {{
            background-color: transparent;
            border: none;
            border-radius: 4px;
            color: alpha(@foreground, 0.55);
            font-size: 14px;
            font-weight: bold;
            min-height: 20px;
            min-width: 20px;
            padding: 0;
            transition: background-color 100ms ease, color 100ms ease;
        }}
        .win-close-btn:hover {{
            background-color: alpha(#e74c3c, 0.85);
            color: #ffffff;
        }}
        /* Thumbnail area. */
        /* No background here: any painted background would square off the
           rounded corners of the thumbnail. The card background shows through. */
        .win-thumb-box {{
            min-height: {thumb_min_h}px;
            min-width: {thumb_min_w}px;
            border-bottom-left-radius: {card_radius}px;
            border-bottom-right-radius: {card_radius}px;
        }}
        /* GTK does not clip widget content to border-radius, so the live
           thumbnail is pre-rounded on CPU (see round_png_corners). This rule
           is only a fallback for static/theme-provided content. */
        .win-thumbnail {{
            border-radius: {card_radius}px;
        }}
        .preview-placeholder {{ opacity: 0.28; }}

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
    ws_radius = (config.radius - 2).max(4),
    preview_radius = config.radius + 4,
    card_radius = (config.radius + 6).max(16),
    card_min_w = if config.compact_preview { 200 } else { 260 },
    thumb_min_h = if config.compact_preview { 108 } else { 160 },
    thumb_min_w = if config.compact_preview { 200 } else { 260 },
    slide_out_transform = match config.position.as_str() {
        "top"   => "translateY(-200px)",
        "left"  => "translateX(-200px)",
        "right" => "translateX(200px)",
        _       => "translateY(200px)",
    },
    );

    css_data.push_str(&default_css);

    if let Some(path) = &config.style
        && path.exists()
            && let Ok(user_css) = std::fs::read_to_string(path) {
                css_data.push_str(&user_css);
            }

    if let Some(mut theme_path) = dirs::config_dir() {
        theme_path.push(format!("rust-dock/themes/{}/style.css", config.current_theme));
        if theme_path.exists()
            && let Ok(theme_css) = std::fs::read_to_string(theme_path) {
                css_data.push_str(&theme_css);
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

    // The preview is its own layer-shell surface. The global gtk.css paints
    // `window { background-color: ... }` at user priority, which beats the
    // transparent rule above and leaves an opaque square slab behind the
    // rounded card. This second provider is scoped to that one window and
    // registered at user priority, so the preview really is rounded while the
    // main dock panel keeps its original look.
    // RADIUS is substituted below so the surface follows the configured radius.
    let surface_css = "\
        window.dock-preview-window,\
        window.dock-preview-window.background,\
        window.dock-preview-window.background.csd,\
        window.dock-preview-window > contents,\
        window.dock-preview-window contents,\
        window.dock-preview-window decoration,\
        window.dock-preview-window decoration:backdrop {\
            background: transparent;\
            background-color: transparent;\
            border: none;\
            box-shadow: none;\
            margin: 0;\
            padding: 0;\
            border-radius: RADIUS;\
        }\
    "
    .replace("RADIUS", &format!("{}px", config.radius + 4));

    SURFACE_PROVIDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provider = slot.get_or_insert_with(|| {
            let prov = CssProvider::new();
            if let Some(display) = gtk4::gdk::Display::default() {
                // THEME priority is the only one above USER, which is where the
                // global gtk.css (window background) lives.
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &prov,
                    gtk4::STYLE_PROVIDER_PRIORITY_THEME,
                );
            }
            prov
        });
        provider.load_from_data(&surface_css);
    });
}
