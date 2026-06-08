use ini::Ini;
use std::path::PathBuf;
use std::fs;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct Config {
    pub current_theme: String,
    pub auto_hide: bool,
    pub resident: bool,
    pub position: String,
    pub icon_size: i32,
    pub padding: i32,
    pub spacing: i32,
    pub radius: i32,
    pub opacity: f32,
    pub full_screen: bool,
    pub exclusive_zone: bool,
    pub output: Option<String>,
    pub launcher_command: String,
    pub style: Option<PathBuf>,
    pub workspaces: u32,
    pub margin_bottom: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    pub margin_top: i32,
    pub no_launcher: bool,
    pub pinned_apps: Vec<String>,
    pub smart_view: bool,
    pub auto_hide_delay: i32,
    pub system_gap_used: bool,
    pub margin: i32,
    pub context_pos: i32,
    pub mode: String,
    pub fps: i32,
    pub buffer_size: i32,
    pub show_delay: i32,
    pub hide_delay: i32,
    pub move_delay: i32,
}

impl Config {
    pub fn new() -> Self {
        let mut config = Config {
            current_theme: "lotos".to_string(),
            auto_hide: false,
            resident: false,
            position: "bottom".to_string(),
            icon_size: 23,
            padding: 4,
            spacing: 5,
            radius: 10,
            opacity: 0.8,
            full_screen: false,
            exclusive_zone: true,
            output: None,
            launcher_command: String::new(),
            style: None,
            workspaces: 10,
            margin_bottom: 0,
            margin_left: 0,
            margin_right: 0,
            margin_top: 0,
            no_launcher: true,
            pinned_apps: Vec::new(),
            smart_view: false,
            auto_hide_delay: 400,
            system_gap_used: true,
            margin: 8,
            context_pos: 5,
            mode: "none".to_string(),
            fps: 30,
            buffer_size: 5,
            show_delay: 500,
            hide_delay: 350,
            move_delay: 100,
        };

        if let Some(mut path) = dirs::config_dir() {
            path.push("rust-dock/hypr-dock.conf");
            if path.exists() {
                if let Ok(ini) = Ini::load_from_file(&path) {
                    if let Some(general) = ini.section(Some("General")) {
                        if let Some(v) = general.get("CurrentTheme") { config.current_theme = v.to_string(); }
                        if let Some(v) = general.get("IconSize") { config.icon_size = v.parse().unwrap_or(23); }
                        if let Some(v) = general.get("Position") { config.position = v.to_string(); }
                        if let Some(v) = general.get("Exclusive") { config.exclusive_zone = v.parse().unwrap_or(true); }
                        if let Some(v) = general.get("SmartView") { config.smart_view = v.parse().unwrap_or(false); }
                        if let Some(v) = general.get("AutoHideDelay") { config.auto_hide_delay = v.parse().unwrap_or(400); }
                        if let Some(v) = general.get("SystemGapUsed") { config.system_gap_used = v.parse().unwrap_or(true); }
                        if let Some(v) = general.get("Margin") { config.margin = v.parse().unwrap_or(8); }
                        if let Some(v) = general.get("ContextPos") { config.context_pos = v.parse().unwrap_or(5); }
                    }
                    if let Some(preview) = ini.section(Some("General.preview")) {
                        if let Some(v) = preview.get("Mode") { config.mode = v.to_string(); }
                        if let Some(v) = preview.get("FPS") { config.fps = v.parse().unwrap_or(30); }
                        if let Some(v) = preview.get("BufferSize") { config.buffer_size = v.parse().unwrap_or(5); }
                        if let Some(v) = preview.get("ShowDelay") { config.show_delay = v.parse().unwrap_or(500); }
                        if let Some(v) = preview.get("HideDelay") { config.hide_delay = v.parse().unwrap_or(350); }
                        if let Some(v) = preview.get("MoveDelay") { config.move_delay = v.parse().unwrap_or(100); }
                    }
                    if let Some(theme) = ini.section(Some("Theme")) {
                        if let Some(v) = theme.get("Spacing") { config.spacing = v.parse().unwrap_or(5); }
                    }
                }
            }
        }

        config.load_pinned_apps();
        config
    }

    pub fn load_pinned_apps(&mut self) {
        self.pinned_apps.clear();
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("rust-dock/pinned");
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        if !line.trim().is_empty() {
                            self.pinned_apps.push(line.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    pub fn save_pinned_apps(&self) {
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("rust-dock");
            let _ = fs::create_dir_all(&path);
            path.push("pinned");
            if let Ok(mut file) = fs::File::create(&path) {
                for app in &self.pinned_apps {
                    let _ = writeln!(file, "{}", app);
                }
            }
        }
    }

    pub fn pin_app(&mut self, app_id: &str) {
        if !self.pinned_apps.contains(&app_id.to_string()) {
            self.pinned_apps.push(app_id.to_string());
            self.save_pinned_apps();
        }
    }

    pub fn unpin_app(&mut self, app_id: &str) {
        self.pinned_apps.retain(|id| id != app_id);
        self.save_pinned_apps();
    }
}
