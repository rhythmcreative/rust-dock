use gtk4::CssProvider;
use std::path::PathBuf;
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
        window {{
            background-color: alpha(@background, {opacity});
            border-radius: {radius}px;
            border: 1px solid alpha(@color1, 0.4);
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
        }}
        button:hover {{
            background-color: alpha(@color1, 0.3);
        }}
        .launcher-btn {{
            color: @color2;
        }}
        .running {{
            border-bottom: 2px solid @color4;
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

    provider.load_from_data(&css_data);

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
