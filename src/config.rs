use ini::Ini;
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use log;

#[derive(Debug, Clone)]
pub struct Config {
    pub current_theme: String,
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
    pub show_delay: i32,
    pub hide_delay: i32,
    pub move_delay: i32,
    pub compact_preview: bool,
    pub sort_running_apps: bool,
}

impl Config {
    pub fn new() -> Self {
        let mut config = Config {
            current_theme: "lotos".to_string(),
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
            show_delay: 20,
            hide_delay: 120,
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
                        if let Some(v) = general.get("Padding") { config.padding = v.parse().unwrap_or(4); }
                        if let Some(v) = general.get("Radius") { config.radius = v.parse().unwrap_or(10); }
                        if let Some(v) = general.get("Opacity") { config.opacity = v.parse().unwrap_or(0.8); }
                        if let Some(v) = general.get("Output") { if !v.trim().is_empty() { config.output = Some(v.to_string()); } }
                        if let Some(v) = general.get("LauncherCommand") { config.launcher_command = v.to_string(); }
                        if let Some(v) = general.get("NoLauncher") { config.no_launcher = v.parse().unwrap_or(true); }
                        if let Some(v) = general.get("Layer") { config.layer = v.to_string(); }
                        if let Some(v) = general.get("FullScreen") { config.full_screen = v.parse().unwrap_or(false); }
                        if let Some(v) = general.get("CompactPreview") { config.compact_preview = v.parse().unwrap_or(false); }
                        if let Some(v) = general.get("SortRunningApps") { config.sort_running_apps = v.parse().unwrap_or(false); }
                    }
                    if let Some(preview) = ini.section(Some("General.preview")) {
                        if let Some(v) = preview.get("ShowDelay") { config.show_delay = v.parse().unwrap_or(500); }
                        if let Some(v) = preview.get("HideDelay") { config.hide_delay = v.parse().unwrap_or(350); }
                        if let Some(v) = preview.get("MoveDelay") { config.move_delay = v.parse().unwrap_or(100); }
                        if let Some(v) = preview.get("Compact") { config.compact_preview = v.parse().unwrap_or(false); }
                    }
                    if let Some(theme) = ini.section(Some("Theme")) {
                        if let Some(v) = theme.get("Spacing") { config.spacing = v.parse().unwrap_or(5); }
                    }
                    // [PinnedApps] section: Apps = firefox,kitty,org.gnome.eog
                    // Used as fallback when the live pinned file doesn't exist yet.
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

        // The live pinned file (modified by UI pin/unpin) takes priority over the
        // INI [PinnedApps] key. Only overwrites when the file actually exists.
        config.load_pinned_apps();
        config.validate_and_clamp();
        config
    }

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
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("rust-dock/pinned");
            if path.exists() {
                // Live file wins over [PinnedApps] in the INI — it's the current UI state.
                self.pinned_apps.clear();
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
        let Some(from)     = self.pinned_apps.iter().position(|a| a == dragged) else { return; };
        let Some(to_orig)  = self.pinned_apps.iter().position(|a| a == target)  else { return; };
        let item = self.pinned_apps.remove(from);
        // After removing `from`, every index > from shifts left by 1.
        // Re-find the target's new index, then compensate: when dragging
        // rightward (from < to_orig) insert *after* the target so the item
        // lands exactly at to_orig instead of one slot short.
        let to = self.pinned_apps
            .iter()
            .position(|a| a == target)
            .unwrap_or(self.pinned_apps.len());
        let insert_at = if from < to_orig { to + 1 } else { to };
        self.pinned_apps.insert(insert_at.min(self.pinned_apps.len()), item);
        self.save_pinned_apps();
    }
}
