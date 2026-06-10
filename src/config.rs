use ini::Ini;
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use log;

#[derive(Debug, Clone)]
pub struct Config {
    pub current_theme: String,
    /// Reserved for future use (not yet wired to behavior).
    #[allow(dead_code)]
    pub auto_hide: bool,
    /// Reserved for future use (not yet wired to behavior).
    #[allow(dead_code)]
    pub resident: bool,
    pub position: String,
    pub icon_size: i32,
    pub padding: i32,
    pub spacing: i32,
    pub radius: i32,
    pub opacity: f32,
    pub full_screen: bool,
    pub exclusive_zone: bool,
    pub layer: String,
    pub output: Option<String>,
    pub launcher_command: String,
    pub style: Option<PathBuf>,
    /// Reserved for future use (not yet wired to behavior).
    #[allow(dead_code)]
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
    /// Show smaller preview cards (200×120 instead of 340×200).
    pub compact_preview: bool,
    /// Sort the running-apps bar alphabetically.
    pub sort_running_apps: bool,
}

impl Config {
    pub fn new() -> Self {
        let mut config = Config {
            current_theme: "lotos".to_string(),
            auto_hide: false,
            resident: false,
            position: "bottom".to_string(),
            icon_size: 23,
            padding: 2,
            spacing: 4,
            radius: 10,
            opacity: 0.8,
            full_screen: false,
            exclusive_zone: true,
            layer: "top".to_string(),
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
            system_gap_used: false,
            margin: 8,
            context_pos: 5,
            mode: "none".to_string(),
            fps: 30,
            buffer_size: 5,
            show_delay: 20,
            hide_delay: 300,
            move_delay: 30,
            compact_preview: false,
            sort_running_apps: false,
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
                        if let Some(v) = general.get("Padding") { config.padding = v.parse().unwrap_or(4); }
                        if let Some(v) = general.get("Radius") { config.radius = v.parse().unwrap_or(10); }
                        if let Some(v) = general.get("Opacity") { config.opacity = v.parse().unwrap_or(0.8); }
                        if let Some(v) = general.get("Output") { if !v.trim().is_empty() { config.output = Some(v.to_string()); } }
                        if let Some(v) = general.get("LauncherCommand") { config.launcher_command = v.to_string(); }
                        if let Some(v) = general.get("NoLauncher") { config.no_launcher = v.parse().unwrap_or(true); }
                        if let Some(v) = general.get("Layer") { config.layer = v.to_string(); }
                        if let Some(v) = general.get("CompactPreview") { config.compact_preview = v.parse().unwrap_or(false); }
                        if let Some(v) = general.get("SortRunningApps") { config.sort_running_apps = v.parse().unwrap_or(false); }
                    }
                    if let Some(preview) = ini.section(Some("General.preview")) {
                        if let Some(v) = preview.get("Mode") { config.mode = v.to_string(); }
                        if let Some(v) = preview.get("FPS") { config.fps = v.parse().unwrap_or(30); }
                        if let Some(v) = preview.get("BufferSize") { config.buffer_size = v.parse().unwrap_or(5); }
                        if let Some(v) = preview.get("ShowDelay") { config.show_delay = v.parse().unwrap_or(500); }
                        if let Some(v) = preview.get("HideDelay") { config.hide_delay = v.parse().unwrap_or(350); }
                        if let Some(v) = preview.get("MoveDelay") { config.move_delay = v.parse().unwrap_or(100); }
                        if let Some(v) = preview.get("Compact") { config.compact_preview = v.parse().unwrap_or(false); }
                    }
                    if let Some(theme) = ini.section(Some("Theme")) {
                        if let Some(v) = theme.get("Spacing") { config.spacing = v.parse().unwrap_or(5); }
                    }
                    // [PinnedApps] section: Apps = firefox,kitty,org.gnome.eog
                    // Takes precedence over the separate ~./local/share/rust-dock/pinned file.
                    if let Some(pinned_sec) = ini.section(Some("PinnedApps")) {
                        if let Some(apps) = pinned_sec.get("Apps") {
                            let from_ini: Vec<String> = apps.split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if !from_ini.is_empty() {
                                config.pinned_apps = from_ini;
                            }
                        }
                    }
                }
            }
        }

        config.load_pinned_apps();
        config.validate_and_clamp();
        config    }

    fn validate_and_clamp(&mut self) {
        self.opacity     = self.opacity.clamp(0.05, 1.0);
        self.icon_size   = self.icon_size.clamp(8, 128);
        self.padding     = self.padding.clamp(0, 64);
        self.radius      = self.radius.clamp(0, 40);
        self.margin      = self.margin.clamp(0, 200);
        self.show_delay  = self.show_delay.clamp(0, 5000);
        self.hide_delay  = self.hide_delay.clamp(0, 5000);
        self.move_delay  = self.move_delay.clamp(0, 2000);
        if !["top", "bottom", "left", "right"].contains(&self.position.as_str()) {
            log::warn!("Invalid position '{}', defaulting to 'bottom'", self.position);
            self.position = "bottom".to_string();
        }
        if !["top", "bottom", "overlay", "background"].contains(&self.layer.as_str()) {
            self.layer = "top".to_string();
        }
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

    /// Move `dragged` to the slot currently occupied by `target` (for drag & drop
    /// reordering of pinned apps), then persist the new order.
    pub fn reorder_pinned(&mut self, dragged: &str, target: &str) {
        if dragged == target { return; }
        let Some(from) = self.pinned_apps.iter().position(|a| a == dragged) else { return; };
        let item = self.pinned_apps.remove(from);
        let to = self.pinned_apps
            .iter()
            .position(|a| a == target)
            .unwrap_or(self.pinned_apps.len());
        self.pinned_apps.insert(to, item);
        self.save_pinned_apps();
    }
}
