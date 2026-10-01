use ini::Ini;
use std::path::PathBuf;
use std::fs;
use std::io::Write;

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

/// Single source of truth for every default value.
///
/// The INI parser falls back to these instead of repeating literals, so a
/// default is only ever written once — changing it here is enough and there
/// is no second copy in the parsing code to drift out of sync.
impl Default for Config {
    fn default() -> Self {
        Config {
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
        }
    }
}

impl Config {
    pub fn new() -> Self {
        let mut config = Config::default();

        if let Some(mut path) = dirs::config_dir() {
            path.push("rust-dock/hypr-dock.conf");
            if path.exists()
                && let Ok(ini) = Ini::load_from_file(&path) {
                    // Fallback values come from Default, never from literals here.
                    let d = Config::default();
                    if let Some(general) = ini.section(Some("General")) {
                        if let Some(v) = general.get("CurrentTheme") { config.current_theme = v.to_string(); }
                        if let Some(v) = general.get("IconSize") { config.icon_size = v.parse().unwrap_or(d.icon_size); }
                        if let Some(v) = general.get("Position") { config.position = v.to_string(); }
                        if let Some(v) = general.get("Exclusive") { config.exclusive_zone = v.parse().unwrap_or(d.exclusive_zone); }
                        if let Some(v) = general.get("SmartView") { config.smart_view = v.parse().unwrap_or(d.smart_view); }
                        if let Some(v) = general.get("AutoHideDelay") { config.auto_hide_delay = v.parse().unwrap_or(d.auto_hide_delay); }
                        if let Some(v) = general.get("SystemGapUsed") { config.system_gap_used = v.parse().unwrap_or(d.system_gap_used); }
                        if let Some(v) = general.get("Margin") { config.margin = v.parse().unwrap_or(d.margin); }
                        if let Some(v) = general.get("Padding") { config.padding = v.parse().unwrap_or(d.padding); }
                        if let Some(v) = general.get("Radius") { config.radius = v.parse().unwrap_or(d.radius); }
                        if let Some(v) = general.get("Opacity") { config.opacity = v.parse().unwrap_or(d.opacity); }
                        if let Some(v) = general.get("Output") && !v.trim().is_empty() { config.output = Some(v.to_string()); }
                        if let Some(v) = general.get("LauncherCommand") { config.launcher_command = v.to_string(); }
                        if let Some(v) = general.get("NoLauncher") { config.no_launcher = v.parse().unwrap_or(d.no_launcher); }
                        if let Some(v) = general.get("Layer") { config.layer = v.to_string(); }
                        if let Some(v) = general.get("FullScreen") { config.full_screen = v.parse().unwrap_or(d.full_screen); }
                        if let Some(v) = general.get("CompactPreview") { config.compact_preview = v.parse().unwrap_or(d.compact_preview); }
                        if let Some(v) = general.get("SortRunningApps") { config.sort_running_apps = v.parse().unwrap_or(d.sort_running_apps); }
                        // Per-edge margins. These used to exist in the struct and
                        // were used in 15 places, but were never parsed here, so
                        // setting them in the INI silently did nothing.
                        if let Some(v) = general.get("MarginTop") { config.margin_top = v.parse().unwrap_or(d.margin_top); }
                        if let Some(v) = general.get("MarginBottom") { config.margin_bottom = v.parse().unwrap_or(d.margin_bottom); }
                        if let Some(v) = general.get("MarginLeft") { config.margin_left = v.parse().unwrap_or(d.margin_left); }
                        if let Some(v) = general.get("MarginRight") { config.margin_right = v.parse().unwrap_or(d.margin_right); }
                    }
                    if let Some(preview) = ini.section(Some("General.preview")) {
                        if let Some(v) = preview.get("ShowDelay") { config.show_delay = v.parse().unwrap_or(d.show_delay); }
                        if let Some(v) = preview.get("HideDelay") { config.hide_delay = v.parse().unwrap_or(d.hide_delay); }
                        if let Some(v) = preview.get("MoveDelay") { config.move_delay = v.parse().unwrap_or(d.move_delay); }
                        if let Some(v) = preview.get("Compact") { config.compact_preview = v.parse().unwrap_or(d.compact_preview); }
                    }
                    if let Some(theme) = ini.section(Some("Theme"))
                        && let Some(v) = theme.get("Spacing") { config.spacing = v.parse().unwrap_or(d.spacing); }
                    // [PinnedApps] section: Apps = firefox,kitty,org.gnome.eog
                    // Used as fallback when the live pinned file doesn't exist yet.
                    if let Some(pinned_sec) = ini.section(Some("PinnedApps"))
                        && let Some(apps) = pinned_sec.get("Apps") {
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
        self.margin_top  = self.margin_top.clamp(0, 4000);
        self.margin_bottom = self.margin_bottom.clamp(0, 4000);
        self.margin_left = self.margin_left.clamp(0, 4000);
        self.margin_right = self.margin_right.clamp(0, 4000);
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

    /// Where the live pinned-app list lives.
    ///
    /// `RUST_DOCK_STATE_DIR` overrides it so tests can never write to the real
    /// user's `pinned` file — an earlier version of the reorder tests did
    /// exactly that and clobbered the live dock state.
    pub fn pinned_path() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("RUST_DOCK_STATE_DIR") {
            return Some(PathBuf::from(dir).join("pinned"));
        }
        dirs::data_local_dir().map(|d| d.join("rust-dock").join("pinned"))
    }

    pub fn load_pinned_apps(&mut self) {
        if let Some(path) = Config::pinned_path()
            && path.exists()
        {
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

    pub fn save_pinned_apps(&self) {
        let Some(path) = Config::pinned_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = fs::File::create(&path) {
            for app in &self.pinned_apps {
                let _ = writeln!(file, "{}", app);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_internally_consistent() {
        let d = Config::default();
        assert_eq!(d.position, "bottom");
        assert_eq!(d.layer, "top");
        // The INI parser used to default this to `true` while the struct said
        // `false`, so a malformed key silently flipped behaviour.
        assert!(!d.system_gap_used);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let mut c = Config::default();
        c.opacity = 5.0;
        c.icon_size = 9999;
        c.margin = -50;
        c.margin_top = -1;
        c.validate_and_clamp();
        assert_eq!(c.opacity, 1.0);
        assert_eq!(c.icon_size, 128);
        assert_eq!(c.margin, 0);
        assert_eq!(c.margin_top, 0);
    }

    #[test]
    fn rejects_invalid_position_and_layer() {
        let mut c = Config::default();
        c.position = "diagonal".to_string();
        c.layer = "nonsense".to_string();
        c.validate_and_clamp();
        assert_eq!(c.position, "bottom");
        assert_eq!(c.layer, "top");
    }

    /// Process-wide lock guarding every test that mutates environment variables.
    ///
    /// `set_var` affects the whole process, so two tests running in parallel
    /// would clobber each other's `RUST_DOCK_STATE_DIR` / `XDG_CONFIG_HOME` and
    /// read the wrong file. One shared lock for both helpers keeps them
    /// mutually exclusive.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Redirects pinned-app persistence to a throwaway directory for the duration
    /// of `f`, so a test can never overwrite the real user's `pinned` file.
    fn with_sandboxed_state<T>(f: impl FnOnce() -> T) -> T {
        let _guard = env_lock();

        let dir = std::env::temp_dir().join(format!(
            "rust-dock-state-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::create_dir_all(&dir);

        let prev = std::env::var_os("RUST_DOCK_STATE_DIR");
        // SAFETY: guarded by `env_lock`, so no other test observes this window.
        unsafe { std::env::set_var("RUST_DOCK_STATE_DIR", &dir) };
        let out = f();
        match prev {
            Some(v) => unsafe { std::env::set_var("RUST_DOCK_STATE_DIR", v) },
            None    => unsafe { std::env::remove_var("RUST_DOCK_STATE_DIR") },
        }
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn reorder_moves_item_rightward_past_target() {
        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pinned_apps = vec!["a".into(), "b".into(), "c".into()];
            c.reorder_pinned("a", "c");
            assert_eq!(c.pinned_apps, vec!["b", "c", "a"]);
        });
    }

    #[test]
    fn reorder_moves_item_leftward_before_target() {
        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pinned_apps = vec!["a".into(), "b".into(), "c".into()];
            c.reorder_pinned("c", "a");
            assert_eq!(c.pinned_apps, vec!["c", "a", "b"]);
        });
    }

    #[test]
    fn reorder_of_missing_item_is_a_noop() {
        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pinned_apps = vec!["a".into(), "b".into()];
            c.reorder_pinned("ghost", "a");
            assert_eq!(c.pinned_apps, vec!["a", "b"]);
        });
    }

    #[test]
    fn pin_and_unpin_round_trip_through_the_state_file() {
        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pin_app("kitty");
            c.pin_app("firefox");

            // A fresh Config must read back what was persisted.
            let mut reloaded = Config::default();
            reloaded.load_pinned_apps();
            assert_eq!(reloaded.pinned_apps, vec!["kitty", "firefox"]);

            // Pinning twice must not duplicate the entry.
            reloaded.pin_app("kitty");
            assert_eq!(reloaded.pinned_apps, vec!["kitty", "firefox"]);

            reloaded.unpin_app("kitty");
            let mut after = Config::default();
            after.load_pinned_apps();
            assert_eq!(after.pinned_apps, vec!["firefox"]);
        });
    }

    #[test]
    fn state_dir_override_is_honoured() {
        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pinned_apps = vec!["only-in-sandbox".into()];
            c.save_pinned_apps();
            let path = Config::pinned_path().expect("path");
            let body = std::fs::read_to_string(&path).unwrap();
            assert!(body.contains("only-in-sandbox"));
        });
    }

    #[test]
    fn sandboxed_write_does_not_touch_the_real_pinned_file() {
        // The regression that motivated `RUST_DOCK_STATE_DIR`: reorder tests
        // used to call save_pinned_apps() against the developer's real
        // ~/.local/share/rust-dock/pinned and overwrote their actual dock.
        let real = dirs::data_local_dir().map(|d| d.join("rust-dock").join("pinned"));
        let real_before = real.as_ref().and_then(|p| std::fs::read_to_string(p).ok());

        with_sandboxed_state(|| {
            let mut c = Config::default();
            c.pinned_apps = vec!["SENTINEL_SHOULD_NOT_ESCAPE".into()];
            c.reorder_pinned("SENTINEL_SHOULD_NOT_ESCAPE", "SENTINEL_SHOULD_NOT_ESCAPE");
        });

        let real_after = real.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        assert_eq!(
            real_before, real_after,
            "a test wrote to the real pinned file"
        );
        assert!(
            !real_after.as_deref().unwrap_or("").contains("SENTINEL_SHOULD_NOT_ESCAPE"),
            "sandboxed value leaked into the real pinned file"
        );
    }

    /// Writes an INI to a private XDG_CONFIG_HOME and parses it, so these
    /// exercise the real `Config::new()` path rather than the struct directly.
    fn parse_ini(contents: &str) -> Config {
        // Shares `env_lock` with `with_sandboxed_state`: both mutate
        // process-wide env vars and must not run concurrently.
        let _guard = env_lock();

        let dir = std::env::temp_dir().join(format!(
            "rust-dock-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let conf = dir.join("rust-dock");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(conf.join("hypr-dock.conf"), contents).unwrap();

        // Isolate both the INI and the live pinned file. `Config::new()` reads
        // the latter from the user's real data dir, which would otherwise leak
        // the developer's actual pinned apps into these assertions.
        let prev_cfg = std::env::var_os("XDG_CONFIG_HOME");
        let prev_state = std::env::var_os("RUST_DOCK_STATE_DIR");
        let state = dir.join("state");
        std::fs::create_dir_all(&state).unwrap();
        // SAFETY: guarded by LOCK; both vars are restored right after.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("RUST_DOCK_STATE_DIR", &state);
        }
        let cfg = Config::new();
        match prev_cfg {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None    => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        match prev_state {
            Some(v) => unsafe { std::env::set_var("RUST_DOCK_STATE_DIR", v) },
            None    => unsafe { std::env::remove_var("RUST_DOCK_STATE_DIR") },
        }
        let _ = std::fs::remove_dir_all(&dir);
        cfg
    }

    #[test]
    fn per_edge_margins_are_parsed_from_ini() {
        // Regression: these four keys were declared and used in 15 places but
        // never parsed, so setting them in the INI silently did nothing.
        let c = parse_ini(
            "[General]\n\
             Margin = 8\n\
             MarginTop = 40\n\
             MarginBottom = 12\n\
             MarginLeft = 25\n\
             MarginRight = 33\n",
        );
        assert_eq!(c.margin, 8);
        assert_eq!(c.margin_top, 40);
        assert_eq!(c.margin_bottom, 12);
        assert_eq!(c.margin_left, 25);
        assert_eq!(c.margin_right, 33);
    }

    #[test]
    fn malformed_margin_key_falls_back_to_default() {
        let c = parse_ini("[General]\nMarginTop = banana\n");
        assert_eq!(c.margin_top, Config::default().margin_top);
    }

    #[test]
    fn ini_overrides_defaults_for_core_keys() {
        let c = parse_ini(
            "[General]\n\
             Position = left\n\
             IconSize = 48\n\
             Opacity = 0.5\n\
             Radius = 4\n\
             Layer = overlay\n\
             FullScreen = true\n\
             NoLauncher = false\n\
             [Theme]\n\
             Spacing = 9\n",
        );
        assert_eq!(c.position, "left");
        assert_eq!(c.icon_size, 48);
        assert_eq!(c.opacity, 0.5);
        assert_eq!(c.radius, 4);
        assert_eq!(c.layer, "overlay");
        assert!(c.full_screen);
        assert!(!c.no_launcher);
        assert_eq!(c.spacing, 9);
    }

    #[test]
    fn blank_output_key_means_all_monitors() {
        // An empty `Output` must not become Some("") and silently never match.
        assert_eq!(parse_ini("[General]\nOutput =\n").output, None);
        assert_eq!(parse_ini("[General]\nOutput = DP-1\n").output.as_deref(), Some("DP-1"));
    }

    #[test]
    fn unknown_section_is_ignored_without_panicking() {
        let c = parse_ini("[Nonsense]\nWhatever = 1\n[General]\nIconSize = 30\n");
        assert_eq!(c.icon_size, 30);
    }
}

