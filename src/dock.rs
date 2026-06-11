use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Image, Orientation,
    Label, EventControllerMotion, Align,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use crate::config::Config;
use crate::hyprland_handler::{HyprlandHandler, HyprClient, capture_window_screenshot};
use crate::app_info::AppInfo;
use std::rc::{Rc, Weak};
use std::cell::{RefCell, Cell};
use std::sync::{mpsc, Arc};


// ── Preview window state shared across all buttons ──────────────────────────
pub struct PreviewState {
    /// The dedicated layer-shell preview window (never destroyed, just shown/hidden)
    pub win:              ApplicationWindow,
    /// Which app class is currently shown (empty = hidden)
    pub active_class:     String,
    /// Whether the preview window is visible right now
    pub visible:          bool,
    /// Which screen edge the dock is on ("bottom", "top", "left", "right")
    pub dock_position:    String,
    /// Smart hide timer of the main dock window
    pub smart_hide_timer: Rc<Cell<Option<glib::SourceId>>>,
    /// Whether smart view is enabled
    pub smart_view:       bool,
    /// Auto hide delay of the main dock window
    pub auto_hide_delay:  u64,
    /// The main dock window
    pub dock_win:         ApplicationWindow,
}

pub struct Dock {
    pub window:           ApplicationWindow,
    pub detect_window:    Option<ApplicationWindow>,
    pub box_container:    Box,
    /// Workspace/taskbar row (non-pinned running apps) appended after pinned icons.
    pub workspace_box:    Box,
    /// Thin separator between pinned icons and the running-apps section.
    pub ws_separator:     gtk4::Separator,
    pub config:           Rc<RefCell<Config>>,
    pub hyprland:         HyprlandHandler,
    /// Weak handle to self so widget callbacks (e.g. pin/unpin) can rebuild the dock.
    self_weak:            RefCell<Weak<Dock>>,
    /// (app id, button) pairs of the current icons, used to toggle the active highlight.
    active_buttons:       RefCell<Vec<(String, Button)>>,
    /// Class of the currently focused app, lowercased.
    active_class:         RefCell<String>,
    pub preview_state:    Rc<RefCell<PreviewState>>,
    pub show_timer:       Rc<Cell<Option<glib::SourceId>>>,
    pub hide_timer:       Rc<Cell<Option<glib::SourceId>>>,
}

fn safe_remove_source(id: glib::SourceId) {
    // glib 0.22 doesn't expose a safe source_remove; use the raw FFI directly.
    unsafe { glib::ffi::g_source_remove(id.as_raw()); }
}

impl Dock {
    pub fn new(app: &Application, config: Rc<RefCell<Config>>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("rust-dock")
            .default_width(1)
            .default_height(1)
            .build();

        window.init_layer_shell();
        window.set_namespace(Some("rust-dock"));
        window.set_decorated(false);

        let cfg = config.borrow();

        let mut layer = match cfg.layer.as_str() {
            "overlay" => Layer::Overlay,
            "bottom" => Layer::Bottom,
            "background" => Layer::Background,
            _ => Layer::Top,
        };
        if cfg.smart_view {
            layer = Layer::Bottom;
        }
        window.set_layer(layer);

        match cfg.position.as_str() {
            "top" => {
                window.set_anchor(Edge::Top, true);
                if cfg.full_screen { window.set_anchor(Edge::Left, true); window.set_anchor(Edge::Right, true); }
            }
            "left" => {
                window.set_anchor(Edge::Left, true);
                if cfg.full_screen { window.set_anchor(Edge::Top, true); window.set_anchor(Edge::Bottom, true); }
            }
            "right" => {
                window.set_anchor(Edge::Right, true);
                if cfg.full_screen { window.set_anchor(Edge::Top, true); window.set_anchor(Edge::Bottom, true); }
            }
            _ => {
                window.set_anchor(Edge::Bottom, true);
                if cfg.full_screen { window.set_anchor(Edge::Left, true); window.set_anchor(Edge::Right, true); }
            }
        }

        if cfg.exclusive_zone && !cfg.smart_view {
            window.auto_exclusive_zone_enable();
        } else {
            window.set_exclusive_zone(-1);
        }

        let mut margin = cfg.margin;
        if cfg.system_gap_used { margin = HyprlandHandler::new().get_gaps_out(); }

        window.set_margin(Edge::Bottom, 0);
        window.set_margin(Edge::Top,    0);
        window.set_margin(Edge::Left,   0);
        window.set_margin(Edge::Right,  0);

        match cfg.position.as_str() {
            "top" => {
                window.set_margin(Edge::Top, cfg.margin_top + margin);
                if cfg.full_screen {
                    window.set_margin(Edge::Left, cfg.margin_left + margin);
                    window.set_margin(Edge::Right, cfg.margin_right + margin);
                }
            }
            "left" => {
                window.set_margin(Edge::Left, cfg.margin_left + margin);
                if cfg.full_screen {
                    window.set_margin(Edge::Top, cfg.margin_top + margin);
                    window.set_margin(Edge::Bottom, cfg.margin_bottom + margin);
                }
            }
            "right" => {
                window.set_margin(Edge::Right, cfg.margin_right + margin);
                if cfg.full_screen {
                    window.set_margin(Edge::Top, cfg.margin_top + margin);
                    window.set_margin(Edge::Bottom, cfg.margin_bottom + margin);
                }
            }
            _ => {
                window.set_margin(Edge::Bottom, cfg.margin_bottom + margin);
                if cfg.full_screen {
                    window.set_margin(Edge::Left, cfg.margin_left + margin);
                    window.set_margin(Edge::Right, cfg.margin_right + margin);
                }
            }
        }

        let orientation = if cfg.position == "left" || cfg.position == "right" {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };

        let box_container = Box::builder()
            .orientation(orientation)
            .spacing(4)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(false)
            .vexpand(false)
            .css_classes(vec!["dock-container".to_string()])
            .build();
        box_container.set_margin_start(4);
        box_container.set_margin_end(4);
        box_container.set_margin_top(2);
        box_container.set_margin_bottom(2);
        window.set_child(Some(&box_container));

        if let Some(monitor_name) = &cfg.output {
            if let Some(display) = gtk4::gdk::Display::default() {
                let monitors = display.monitors();
                for i in 0..monitors.n_items() {
                    if let Some(m) = monitors.item(i).and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
                        if m.connector().map(|c| c.to_string()).as_deref() == Some(monitor_name) {
                            window.set_monitor(Some(&m)); break;
                        }
                    }
                }
            }
        }

        let smart_view       = cfg.smart_view;
        let auto_hide_delay  = cfg.auto_hide_delay as u64;
        let dock_position    = cfg.position.clone();
        // smart-view always needs Bottom so the dock can hide behind windows.
        if smart_view { window.set_layer(Layer::Bottom); }

        // ── smart-view detect window ─────────────────────────────────────
        let smart_hide_timer: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
        let mut detect_window = None;

        if smart_view {
            let det_win = ApplicationWindow::builder()
                .application(app)
                .title("dock-detect")
                .default_width(1)
                .default_height(1)
                .build();
            det_win.init_layer_shell();
            det_win.set_namespace(Some("dock-detect"));
            det_win.set_layer(Layer::Top);

            match cfg.position.as_str() {
                "top" => {
                    det_win.set_anchor(Edge::Top, true);
                    det_win.set_anchor(Edge::Left, true);
                    det_win.set_anchor(Edge::Right, true);
                    det_win.set_margin(Edge::Top, 0);
                }
                "left" => {
                    det_win.set_anchor(Edge::Left, true);
                    det_win.set_anchor(Edge::Top, true);
                    det_win.set_anchor(Edge::Bottom, true);
                    det_win.set_margin(Edge::Left, 0);
                }
                "right" => {
                    det_win.set_anchor(Edge::Right, true);
                    det_win.set_anchor(Edge::Top, true);
                    det_win.set_anchor(Edge::Bottom, true);
                    det_win.set_margin(Edge::Right, 0);
                }
                _ => {
                    det_win.set_anchor(Edge::Bottom, true);
                    det_win.set_anchor(Edge::Left, true);
                    det_win.set_anchor(Edge::Right, true);
                    det_win.set_margin(Edge::Bottom, 0);
                }
            }

            if let Some(monitor_name) = &cfg.output {
                if let Some(display) = gtk4::gdk::Display::default() {
                    let monitors = display.monitors();
                    for i in 0..monitors.n_items() {
                        if let Some(m) = monitors.item(i).and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
                            if m.connector().map(|c| c.to_string()).as_deref() == Some(monitor_name) {
                                det_win.set_monitor(Some(&m));
                                break;
                            }
                        }
                    }
                }
            }

            let motion_det  = EventControllerMotion::new();
            let st_f        = Rc::clone(&smart_hide_timer);
            let wc_f        = window.clone();
            motion_det.connect_enter(move |_, _, _| {
                if let Some(id) = st_f.take() { safe_remove_source(id); }
                wc_f.set_layer(Layer::Top);
            });
            let st_u  = Rc::clone(&smart_hide_timer);
            let wc_u  = window.clone();
            motion_det.connect_leave(move |_| {
                if let Some(id) = st_u.take() { safe_remove_source(id); }
                let wc = wc_u.clone();
                let ti = Rc::clone(&st_u);
                let id = glib::timeout_add_local(std::time::Duration::from_millis(auto_hide_delay), move || {
                    wc.set_layer(Layer::Bottom); ti.set(None); glib::ControlFlow::Break
                });
                st_u.set(Some(id));
            });
            det_win.add_controller(motion_det);
            det_win.show();
            detect_window = Some(det_win);
        }

        // ── dock window enter/leave (smart-view) ─────────────────────────
        let motion = EventControllerMotion::new();
        let wc1 = window.clone();
        let st_f2 = Rc::clone(&smart_hide_timer);
        motion.connect_enter(move |_, _, _| {
            if smart_view {
                if let Some(id) = st_f2.take() { safe_remove_source(id); }
                wc1.set_layer(Layer::Top);
            }
        });
        let wc2  = window.clone();
        let st_u2 = Rc::clone(&smart_hide_timer);
        motion.connect_leave(move |_| {
            if smart_view {
                if let Some(id) = st_u2.take() { safe_remove_source(id); }
                let wc = wc2.clone();
                let ti = Rc::clone(&st_u2);
                let id = glib::timeout_add_local(std::time::Duration::from_millis(auto_hide_delay), move || {
                    wc.set_layer(Layer::Bottom); ti.set(None); glib::ControlFlow::Break
                });
                st_u2.set(Some(id));
            }
        });
        window.add_controller(motion);

        // ── Dedicated preview window ─────────────────────────────────────
        // Layer::Overlay so it floats above everything.
        let preview_win = ApplicationWindow::builder()
            .application(app)
            .title("dock-preview")
            .default_width(1)
            .default_height(1)
            .build();
        preview_win.init_layer_shell();
        preview_win.set_namespace(Some("dock-preview"));
        preview_win.set_layer(Layer::Overlay);
        preview_win.set_exclusive_zone(-1);
        preview_win.set_decorated(false);

        match dock_position.as_str() {
            "top"   => { preview_win.set_anchor(Edge::Top, true); preview_win.set_anchor(Edge::Left, true); }
            "left"  => { preview_win.set_anchor(Edge::Left, true); preview_win.set_anchor(Edge::Top, true); }
            "right" => { preview_win.set_anchor(Edge::Right, true); preview_win.set_anchor(Edge::Top, true); }
            _       => { preview_win.set_anchor(Edge::Bottom, true); preview_win.set_anchor(Edge::Left, true); }
        }

        if let Some(monitor_name) = &cfg.output {
            if let Some(display) = gtk4::gdk::Display::default() {
                let monitors = display.monitors();
                for i in 0..monitors.n_items() {
                    if let Some(m) = monitors.item(i).and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
                        if m.connector().map(|c| c.to_string()).as_deref() == Some(monitor_name) {
                            preview_win.set_monitor(Some(&m));
                            break;
                        }
                    }
                }
            }
        }
        preview_win.add_css_class("dock-preview-window");

        let show_timer: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
        let hide_timer: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
        let preview_state = Rc::new(RefCell::new(PreviewState {
            win:              preview_win.clone(),
            active_class:     String::new(),
            visible:          false,
            dock_position:    dock_position.clone(),
            smart_hide_timer: Rc::clone(&smart_hide_timer),
            smart_view,
            auto_hide_delay,
            dock_win:         window.clone(),
        }));

        // Motion controller on the preview window itself.
        //
        // Problem: the preview is a Layer::Overlay surface that sits just above
        // the dock. When the cursor moves upward from the dock it crosses this
        // surface, triggering `connect_enter`. If we simply cancelled the
        // hide_timer there the preview would never hide (the hide_timer set by
        // the button's leave handler is gone and nothing restarts it).
        //
        // Fix: instead of fully cancelling the hide_timer we replace it with a
        // longer-lived one (1 500 ms). This keeps the preview alive while the
        // user intentionally hovers over a preview card to click it, but
        // guarantees it will disappear even if the cursor only crossed the
        // surface accidentally. When the cursor leaves the preview we trim the
        // delay down to 80 ms so the hide feels instant.
        let ht_preview  = Rc::clone(&hide_timer);
        let st_preview  = Rc::clone(&show_timer);
        let ps_preview  = Rc::clone(&preview_state);
        let pv_motion   = EventControllerMotion::new();
        pv_motion.connect_enter(move |_, _, _| {
            if let Some(id) = st_preview.take() { safe_remove_source(id); }

            // Cancel the existing hide_timer and start a longer replacement so
            // the preview stays alive while the user interacts with a card.
            if let Some(id) = ht_preview.take() { safe_remove_source(id); }
            let ps2 = Rc::clone(&ps_preview);
            let ht2 = Rc::clone(&ht_preview);
            let id = glib::timeout_add_local(std::time::Duration::from_millis(1500), move || {
                ht2.set(None);
                let mut s = ps2.borrow_mut();
                s.win.hide();
                s.visible = false;
                s.active_class = String::new();
                glib::ControlFlow::Break
            });
            ht_preview.set(Some(id));

            let ps = ps_preview.borrow();
            if ps.smart_view {
                if let Some(id) = ps.smart_hide_timer.take() { safe_remove_source(id); }
                ps.dock_win.set_layer(Layer::Top);
            }
        });
        let ps_leave    = Rc::clone(&preview_state);
        let ht_leave    = Rc::clone(&hide_timer);
        pv_motion.connect_leave(move |_| {
            // Cursor left the preview — cancel the 1 500 ms stay-timer and
            // replace it with a short one so the hide feels immediate.
            if let Some(id) = ht_leave.take() { safe_remove_source(id); }
            let ps  = Rc::clone(&ps_leave);
            let ht  = Rc::clone(&ht_leave);
            let id = glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
                let mut s = ps.borrow_mut();
                s.win.hide();
                s.visible = false;
                s.active_class = String::new();
                ht.set(None);

                if s.smart_view {
                    if let Some(id) = s.smart_hide_timer.take() { safe_remove_source(id); }
                    let wc = s.dock_win.clone();
                    let ti = Rc::clone(&s.smart_hide_timer);
                    let id = glib::timeout_add_local(std::time::Duration::from_millis(s.auto_hide_delay), move || {
                        wc.set_layer(Layer::Bottom); ti.set(None); glib::ControlFlow::Break
                    });
                    s.smart_hide_timer.set(Some(id));
                }

                glib::ControlFlow::Break
            });
            ht_leave.set(Some(id));
        });
        preview_win.add_controller(pv_motion);
        preview_win.set_focusable(false);

        drop(cfg);

        // ── Workspace bar ─────────────────────────────────────────────────
        // Placed after the app icons, separated by a thin divider.
        let ws_orientation = if dock_position == "left" || dock_position == "right" {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };
        let workspace_box = Box::builder()
            .orientation(ws_orientation)
            .spacing(4)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .css_classes(vec!["workspace-bar".to_string()])
            .build();

        // Create the separator between pinned and running sections.
        // Both are hidden initially; refresh_workspaces shows them as needed.
        let ws_separator = gtk4::Separator::new(if ws_orientation == Orientation::Vertical {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        });
        ws_separator.add_css_class("dock-ws-separator");
        // Append separator + workspace_box to dock container permanently.
        box_container.append(&ws_separator);
        box_container.append(&workspace_box);
        // Start hidden — shown only when non-pinned apps are running.
        ws_separator.hide();
        workspace_box.hide();

        window.show();

        Self {
            window,
            detect_window,
            box_container,
            workspace_box,
            ws_separator,
            config,
            hyprland: HyprlandHandler::new(),
            self_weak: RefCell::new(Weak::new()),
            active_buttons: RefCell::new(Vec::new()),
            active_class: RefCell::new(String::new()),
            preview_state,
            show_timer,
            hide_timer,
        }
    }

    /// Wire up the self-reference and build the initial contents. Must be called
    /// once after the `Dock` is wrapped in an `Rc`.
    pub fn init(self: &Rc<Self>) {
        *self.self_weak.borrow_mut() = Rc::downgrade(self);
        self.refresh();
        self.refresh_workspaces();
    }

    pub fn toggle_visibility(&self) {
        if self.window.is_visible() {
            self.window.hide();
            if let Some(ref det_win) = self.detect_window { det_win.hide(); }
        } else {
            self.window.show();
            if let Some(ref det_win) = self.detect_window { det_win.show(); }
        }
    }

    pub fn set_dock_visible(&self, visible: bool) {
        if visible {
            self.window.show();
            if let Some(ref det_win) = self.detect_window { det_win.show(); }
        } else {
            self.window.hide();
            if let Some(ref det_win) = self.detect_window { det_win.hide(); }
        }
    }

    pub fn refresh(&self) {
        self.refresh_with_preview();
        self.refresh_workspaces();
        // Refresh the focused-app highlight after rebuilding the icons.
        if let Some(class) = self.hyprland.get_active_class() {
            *self.active_class.borrow_mut() = class;
        }
        let active = self.active_class.borrow().clone();
        self.apply_active(&active);
        self.update_exclusive_zone();
    }

    /// Update which icon is highlighted as the focused app (cheap; no rebuild).
    /// If the focused app isn't on the dock yet, a new window appeared (the open
    /// event may have raced), so rebuild to pick it up.
    pub fn update_active(&self, class: &str) {
        *self.active_class.borrow_mut() = class.to_string();
        let known = self.active_buttons
            .borrow()
            .iter()
            .any(|(id, _)| id.eq_ignore_ascii_case(class));
        if !class.is_empty() && !known {
            self.refresh();
        } else {
            self.apply_active(class);
        }
    }

    fn apply_active(&self, class: &str) {
        for (id, btn) in self.active_buttons.borrow().iter() {
            if id.eq_ignore_ascii_case(class) {
                btn.add_css_class("active");
            } else {
                btn.remove_css_class("active");
            }
        }
    }

    fn update_exclusive_zone(&self) {
        let win_clone = self.window.clone();
        let config_clone = Rc::clone(&self.config);
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(50),
            move || {
                let cfg = config_clone.borrow();
                if cfg.exclusive_zone && !cfg.smart_view {
                    let mut margin = cfg.margin;
                    if cfg.system_gap_used { margin = HyprlandHandler::new().get_gaps_out(); }
                    let gaps_out = HyprlandHandler::new().get_gaps_out();
                    let height = win_clone.height();
                    let exclusive_size = height + 2 * margin - gaps_out;
                    win_clone.set_exclusive_zone(exclusive_size.max(0));
                } else {
                    win_clone.set_exclusive_zone(-1);
                }
            }
        );
    }

    fn refresh_with_preview(&self) {
        self.active_buttons.borrow_mut().clear();
        // Window indices are stale after any rebuild, so reset cycling state.
        CYCLE_IDX.with(|m| m.borrow_mut().clear());
        while let Some(child) = self.box_container.first_child() {
            self.box_container.remove(&child);
        }

        if !self.config.borrow().no_launcher {
            let launcher_btn = Button::builder()
                .icon_name("start-here-symbolic")
                .css_classes(vec!["launcher-btn".to_string()])
                .build();
            let cmd = self.config.borrow().launcher_command.clone();
            launcher_btn.connect_clicked(move |_| {
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
            });
            self.box_container.append(&launcher_btn);
        }

        // app_counts: instances per class key (lowercase) — used for pinned app indicators.
        let mut app_counts: std::collections::HashMap<String, usize> = Default::default();

        for client in self.hyprland.get_clients() {
            let class = client.class.clone();
            if class.is_empty() { continue; }
            let class_key = class.to_lowercase();
            *app_counts.entry(class_key).or_insert(0) += 1;
        }

        let pinned_apps = self.config.borrow().pinned_apps.clone();

        for app_id in &pinned_apps {
            if let Some(app) = AppInfo::find_by_id(app_id) {
                // Remove by the class key (lowercase) — that is what app_counts uses.
                // Try app.id first; if that misses, try the raw app_id lowercased
                // (covers the case where the .desktop filename differs from the class).
                let app_id_lower = app_id.to_lowercase();
                let instances = app_counts
                    .remove(&app.id)
                    .or_else(|| app_counts.remove(&app_id_lower))
                    .unwrap_or(0);
                self.add_app_button(&app, instances, true,
                    Rc::clone(&self.preview_state),
                    Rc::clone(&self.show_timer),
                    Rc::clone(&self.hide_timer),
                    Rc::clone(&self.config),
                    &self.box_container,
                );
            }
        }
        // Non-pinned running apps are shown in the workspace_box section below.
        // Do NOT add them here to avoid duplicates.

        self.box_container.append(&self.ws_separator);
        self.box_container.append(&self.workspace_box);

        self.box_container.queue_resize();
        self.window.queue_resize();
    }

    /// Rebuild the "running apps" section (non-pinned apps in execution)
    /// shown below the pinned dock icons, like a taskbar.
    pub fn refresh_workspaces(&self) {
        // Clear old buttons
        while let Some(child) = self.workspace_box.first_child() {
            self.workspace_box.remove(&child);
        }

        // Resolve the dock's monitor index once (for multi-monitor filtering).
        let dock_monitor_id: Option<i32> = self.config.borrow().output.as_ref()
            .and_then(|name| self.hyprland.get_monitor_id(name));

        // Gather running apps — optionally restricted to the dock's monitor.
        let mut app_counts: std::collections::HashMap<String, usize> = Default::default();
        let mut running_ordered: Vec<(String, AppInfo)> = Vec::new();
        let mut all_clients = self.hyprland.get_clients();

        // Multi-monitor: own-monitor windows first, then others.
        if let Some(mon_id) = dock_monitor_id {
            all_clients.sort_by_key(|c| if c.monitor == mon_id { 0 } else { 1 });
        }

        for client in &all_clients {
            let class = client.class.clone();
            if class.is_empty() { continue; }
            let class_key = class.to_lowercase();
            let app = AppInfo::find_by_class(&class).unwrap_or_else(|| AppInfo {
                id: class_key.clone(),
                name: if client.title.is_empty() { class.clone() } else { client.title.clone() },
                icon: Some("application-x-executable".to_string()),
                exec: "".to_string(),
            });
            if !app_counts.contains_key(&class_key) {
                running_ordered.push((class_key.clone(), app));
            }
            *app_counts.entry(class_key).or_insert(0) += 1;
        }

        // Sort alphabetically if configured.
        if self.config.borrow().sort_running_apps {
            running_ordered.sort_by(|(_, a), (_, b)| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        let pinned_apps = self.config.borrow().pinned_apps.clone();
        let pinned_lower: std::collections::HashSet<String> =
            pinned_apps.iter().map(|s| s.to_lowercase()).collect();

        // Collect non-pinned running apps
        let mut has_running = false;
        for (class_key, app) in &running_ordered {
            if pinned_lower.contains(class_key) || pinned_lower.contains(&app.id) {
                continue;
            }
            let instances = match app_counts.get(class_key) {
                Some(&n) => n,
                None => continue,
            };

            self.add_app_button(app, instances, false,
                Rc::clone(&self.preview_state),
                Rc::clone(&self.show_timer),
                Rc::clone(&self.hide_timer),
                Rc::clone(&self.config),
                &self.workspace_box,
            );
            has_running = true;
        }

        if has_running {
            self.ws_separator.show();
            self.workspace_box.show();
        } else {
            self.ws_separator.hide();
            self.workspace_box.hide();
        }

        self.workspace_box.queue_resize();
        self.box_container.queue_resize();
        self.window.queue_resize();
        self.update_exclusive_zone();
    }

    #[allow(clippy::too_many_arguments)]
    fn add_app_button(
        &self,
        app:          &AppInfo,
        instances:    usize,
        pinned:       bool,
        preview_state: Rc<RefCell<PreviewState>>,
        show_timer:   Rc<Cell<Option<glib::SourceId>>>,
        hide_timer:   Rc<Cell<Option<glib::SourceId>>>,
        config:       Rc<RefCell<Config>>,
        container:    &Box,
    ) {
        let btn = Button::builder()
            .css_classes(vec![if pinned { "pinned" } else { "running" }.to_string()])
            .build();

        let vbox = Box::builder().orientation(Orientation::Vertical).spacing(2).build();
        if let Some(icon_name) = &app.icon {
            let img = create_icon_image(icon_name);
            img.set_pixel_size(self.config.borrow().icon_size);
            vbox.append(&img);
        } else {
            vbox.append(&Label::new(Some(&app.name)));
        }
        let badge = if instances > 0 {
            "•".repeat(instances.min(3))
        } else {
            "•".to_string()
        };
        let indicator = Label::builder()
            .label(&badge)
            .css_classes(vec!["indicator".to_string()])
            .build();
        if instances == 0 {
            indicator.set_opacity(0.0);
        }
        vbox.append(&indicator);
        btn.set_child(Some(&vbox));
        btn.set_tooltip_text(Some(&app.name));

        let show_delay = config.borrow().show_delay as u64;
        let hide_delay = config.borrow().hide_delay as u64;
        let move_delay = config.borrow().move_delay as u64;

        // ── Motion: hover in ─────────────────────────────────────────────
        let motion = EventControllerMotion::new();
        {
            let btn_e   = btn.clone();
            let class_e = app.id.clone();
            let icon_e  = app.icon.clone();
            let ps_e    = Rc::clone(&preview_state);
            let st_e    = Rc::clone(&show_timer);
            let ht_e    = Rc::clone(&hide_timer);
            let config_e = Rc::clone(&config);

            motion.connect_enter(move |_, _, _| {
                if let Some(id) = ht_e.take() { safe_remove_source(id); }

                let state_visible;
                let state_class;
                {
                    let s = ps_e.borrow();
                    state_visible = s.visible;
                    state_class   = s.active_class.clone();
                }

                if state_visible {
                    if state_class == class_e {
                        return;
                    }
                    if let Some(id) = st_e.take() { safe_remove_source(id); }

                    let ps  = Rc::clone(&ps_e);
                    let cls = class_e.clone();
                    let icn = icon_e.clone();
                    let btn = btn_e.clone();
                    let cfg_c = Rc::clone(&config_e);
                    let st_c = Rc::clone(&st_e);
                    let id = glib::timeout_add_local(
                        std::time::Duration::from_millis(move_delay),
                        move || {
                            st_c.set(None);
                            update_preview_content(&ps, &cls, &icn, &btn, &cfg_c.borrow());
                            glib::ControlFlow::Break
                        }
                    );
                    st_e.set(Some(id));
                } else {
                    if st_e.borrow_peek() { return; }

                    let ps    = Rc::clone(&ps_e);
                    let cls   = class_e.clone();
                    let icn   = icon_e.clone();
                    let btn   = btn_e.clone();
                    let st_r  = Rc::clone(&st_e);
                    let cfg_c = Rc::clone(&config_e);
                    let id = glib::timeout_add_local(
                        std::time::Duration::from_millis(show_delay),
                        move || {
                            st_r.set(None);
                            let handler = HyprlandHandler::new();
                            if handler.get_clients_for_class(&cls).is_empty() {
                                return glib::ControlFlow::Break;
                            }
                            update_preview_content(&ps, &cls, &icn, &btn, &cfg_c.borrow());
                            glib::ControlFlow::Break
                        }
                    );
                    st_e.set(Some(id));
                }
            });
        }

        // ── Motion: hover out ────────────────────────────────────────────
        {
            let ps_l  = Rc::clone(&preview_state);
            let st_l  = Rc::clone(&show_timer);
            let ht_l  = Rc::clone(&hide_timer);

            motion.connect_leave(move |_| {
                if let Some(id) = st_l.take() { safe_remove_source(id); }

                let visible = ps_l.borrow().visible;
                if !visible { return; }

                let ps  = Rc::clone(&ps_l);
                let ht  = Rc::clone(&ht_l);
                let id = glib::timeout_add_local(
                    std::time::Duration::from_millis(hide_delay),
                    move || {
                        ht.set(None);
                        let mut s = ps.borrow_mut();
                        s.win.hide();
                        s.visible = false;
                        s.active_class = String::new();
                        glib::ControlFlow::Break
                    }
                );
                ht_l.set(Some(id));
            });
        }
        btn.add_controller(motion);

        // ── Right-click context menu ─────────────────────────────────────
        {
            let menu_pop = gtk4::Popover::builder()
                .position(gtk4::PositionType::Top)
                .has_arrow(false)
                .build();
            menu_pop.add_css_class("context-popover");
            menu_pop.set_parent(&btn);
            // Lift the menu a little higher above the icon.
            menu_pop.set_offset(0, -8);

            let mp_d = menu_pop.clone();
            btn.connect_destroy(move |_| { mp_d.unparent(); });

            let menu_box = Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(0)
                .build();

            let app_id_m    = app.id.clone();
            let app_class_m = app.id.clone();
            let app_exec_m  = app.exec.clone();
            let config_m    = Rc::clone(&config);
            let ps_m        = Rc::clone(&preview_state);
            let st_m        = Rc::clone(&show_timer);
            let ht_m        = Rc::clone(&hide_timer);

            // ── Header: app icon + name ──────────────────────────────────
            let header = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(10)
                .css_classes(vec!["ctx-header".to_string()])
                .build();
            let header_icon = create_icon_image(
                app.icon.as_deref().unwrap_or("application-x-executable"),
            );
            header_icon.set_pixel_size(18);
            header.append(&header_icon);
            let header_label = Label::builder()
                .label(&app.name)
                .xalign(0.0)
                .halign(Align::Start)
                .hexpand(true)
                .max_width_chars(22)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .css_classes(vec!["ctx-header-label".to_string()])
                .build();
            header.append(&header_label);
            menu_box.append(&header);
            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // ── New window ───────────────────────────────────────────────
            let new_win_item = ctx_menu_item("list-add-symbolic", "New window", false);
            let exec = app_exec_m.clone();
            new_win_item.connect_clicked(move |_| {
                if !exec.is_empty() {
                    if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&exec).spawn() {
                        log::warn!("Failed to launch new window: {e}");
                    }
                }
            });
            menu_box.append(&new_win_item);

            // ── Pin / Unpin ─────────────────────────────────────────────
            let pin_txt = if pinned { "Unpin from taskbar" } else { "Pin to taskbar" };
            let pin_item = ctx_menu_item("view-pin-symbolic", pin_txt, false);
            let cfg_c = Rc::clone(&config_m);
            let id_c  = app_id_m.clone();
            let pop_r = menu_pop.clone();
            let dock_weak = self.self_weak.borrow().clone();
            pin_item.connect_clicked(move |_| {
                {
                    let mut c = cfg_c.borrow_mut();
                    if pinned { c.unpin_app(&id_c); } else { c.pin_app(&id_c); }
                }
                pop_r.popdown();
                // Rebuild the dock after the click finishes so the icon
                // appears/disappears immediately, without destroying the button
                // mid-signal.
                let dw = dock_weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(dock) = dw.upgrade() { dock.refresh(); }
                });
            });
            menu_box.append(&pin_item);

            // ── Close all windows ──────────────────────────────────────
            if instances > 0 {
                menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));
                let close_item = ctx_menu_item("window-close-symbolic", "Close all windows", true);
                let class_c = app_class_m.clone();
                let pop_r2  = menu_pop.clone();
                close_item.connect_clicked(move |_| {
                    if let Err(e) = std::process::Command::new("hyprctl")
                        .args(["dispatch", "closewindow", &format!("^{}$", class_c)])
                        .spawn()
                    {
                        log::warn!("Failed to close windows for {}: {e}", class_c);
                    }
                    pop_r2.popdown();
                });
                menu_box.append(&close_item);
            }

            menu_pop.set_child(Some(&menu_box));

            menu_pop.connect_show(move |_| {
                if let Some(id) = st_m.take() { safe_remove_source(id); }
                if let Some(id) = ht_m.take() { safe_remove_source(id); }
                {
                    let mut s = ps_m.borrow_mut();
                    s.win.hide();
                    s.visible = false;
                    s.active_class = String::new();
                }
            });

            let rclick   = gtk4::GestureClick::new();
            rclick.set_button(3);
            let mp_click = menu_pop.clone();
            rclick.connect_pressed(move |_, _, _, _| { mp_click.popup(); });
            btn.add_controller(rclick);
        }

        // ── Left-click: focus existing windows (cycling) or launch ───────
        let app_clone = app.clone();
        btn.connect_clicked(move |_| { focus_or_launch(&app_clone); });

        // ── Drag & drop to reorder pinned apps ───────────────────────────
        if pinned {
            let drag = gtk4::DragSource::new();
            drag.set_actions(gtk4::gdk::DragAction::MOVE);
            let drag_id = app.id.clone();
            drag.connect_prepare(move |_, _, _| {
                Some(gtk4::gdk::ContentProvider::for_value(&drag_id.to_value()))
            });
            // Visual feedback: fade the icon being dragged.
            let btn_drag_begin = btn.clone();
            drag.connect_drag_begin(move |_, _| {
                btn_drag_begin.add_css_class("dragging");
            });
            let btn_drag_end = btn.clone();
            drag.connect_drag_end(move |_, _, _| {
                btn_drag_end.remove_css_class("dragging");
            });
            btn.add_controller(drag);

            let drop = gtk4::DropTarget::new(glib::Type::STRING, gtk4::gdk::DragAction::MOVE);
            let target_id = app.id.clone();
            let cfg_drop = Rc::clone(&config);
            let dock_weak = self.self_weak.borrow().clone();
            // Visual feedback: highlight the drop target while hovering over it.
            let btn_motion = btn.clone();
            drop.connect_motion(move |_, _, _| {
                btn_motion.add_css_class("drop-target");
                gtk4::gdk::DragAction::MOVE
            });
            let btn_leave = btn.clone();
            drop.connect_leave(move |_| {
                btn_leave.remove_css_class("drop-target");
            });
            drop.connect_drop(move |tgt, value, _, _| {
                if let Some(w) = tgt.widget() { w.remove_css_class("drop-target"); }
                if let Ok(dragged) = value.get::<String>() {
                    {
                        let mut c = cfg_drop.borrow_mut();
                        c.reorder_pinned(&dragged, &target_id);
                    }
                    let dw = dock_weak.clone();
                    glib::idle_add_local_once(move || {
                        if let Some(dock) = dw.upgrade() { dock.refresh(); }
                    });
                    return true;
                }
                false
            });
            btn.add_controller(drop);
        }

        self.active_buttons.borrow_mut().push((app.id.clone(), btn.clone()));
        container.append(&btn);
    }
}

thread_local! {
    /// Per-app index so repeated clicks cycle through the app's open windows.
    static CYCLE_IDX: RefCell<std::collections::HashMap<String, usize>> =
        RefCell::new(std::collections::HashMap::new());
}


/// Focus an existing window of the app (cycling through them on repeated clicks);
/// if none are open, launch a new instance.
fn focus_or_launch(app: &AppInfo) {
    let handler = HyprlandHandler::new();
    let windows = handler.get_clients_for_class(&app.id);
    if windows.is_empty() {
        app.launch();
        return;
    }
    let idx = CYCLE_IDX.with(|m| {
        let mut m = m.borrow_mut();
        let entry = m.entry(app.id.clone()).or_insert(0);
        let cur = *entry % windows.len();
        *entry = (*entry + 1) % windows.len();
        cur
    });
    let addr = windows[idx].address.clone();
    if let Err(e) = std::process::Command::new("hyprctl")
        .args(["dispatch", "focuswindow", &format!("address:{}", addr)])
        .spawn()
    {
        log::warn!("Failed to focus window {}: {e}", addr);
    }
}

// ── Preview window content management ───────────────────────────────────────

/// Build a single context-menu entry: a leading symbolic icon + left-aligned
/// label inside a flat button, matching `.ctx-menu-item`. `close_style` adds the
/// red close accent.
fn ctx_menu_item(icon_name: &str, label: &str, close_style: bool) -> Button {
    let row = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    let icon = create_icon_image(icon_name);
    icon.set_pixel_size(15);
    icon.add_css_class("ctx-menu-icon");
    row.append(&icon);

    let lbl = Label::builder()
        .label(label)
        .xalign(0.0)
        .halign(Align::Start)
        .hexpand(true)
        .build();
    row.append(&lbl);

    let mut classes = vec!["ctx-menu-item".to_string()];
    if close_style { classes.push("ctx-close-item".to_string()); }
    Button::builder()
        .hexpand(true)
        .css_classes(classes)
        .child(&row)
        .build()
}

fn build_preview_content(
    windows:  &[HyprClient],
    icon:     &Option<String>,
    dock_pos: &str,
    thumb_w:  i32,
    thumb_h:  i32,
) -> Box {
    let orientation = match dock_pos {
        "left" | "right" => Orientation::Vertical,
        _ => Orientation::Horizontal,
    };
    let row_classes = vec!["preview-row".to_string(), format!("preview-row-{}", dock_pos)];
    let cards_row = Box::builder()
        .orientation(orientation)
        .spacing(8)
        .css_classes(row_classes)
        .build();

    for win in windows {
        // Card: vertical Box clipped to border-radius via Overflow::Hidden.
        // Structure: header row on top, thumbnail below — same as Windows taskbar preview.
        let card = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .css_classes(vec!["win-card".to_string()])
            .build();
        card.set_overflow(gtk4::Overflow::Hidden);

        // ── Header: icon + title + close button ──────────────────────────
        let header = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .css_classes(vec!["win-header".to_string()])
            .build();

        let app_icon = create_icon_image(icon.as_deref().unwrap_or("application-x-executable"));
        app_icon.set_pixel_size(14);
        app_icon.set_valign(Align::Center);
        header.append(&app_icon);

        let title_str = if win.title.chars().count() > 26 {
            format!("{}…", win.title.chars().take(26).collect::<String>())
        } else { win.title.clone() };
        let title_lbl = Label::builder()
            .label(&title_str)
            .hexpand(true)
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(vec!["win-title".to_string()])
            .build();
        header.append(&title_lbl);

        let close_x = Button::builder()
            .label("×")
            .valign(Align::Center)
            .css_classes(vec!["win-close-btn".to_string()])
            .build();
        let addr_close = win.address.clone();
        close_x.connect_clicked(move |_| {
            let _ = std::process::Command::new("hyprctl")
                .args(["dispatch", "closewindow", &format!("address:{}", addr_close)])
                .spawn();
        });
        header.append(&close_x);
        card.append(&header);

        // ── Thumbnail ────────────────────────────────────────────────────
        let thumb = Box::builder()
            .orientation(Orientation::Vertical)
            .halign(Align::Fill)
            .valign(Align::Fill)
            .hexpand(true)
            .vexpand(true)
            .css_classes(vec!["win-thumb-box".to_string()])
            .build();
        thumb.set_size_request(thumb_w, thumb_h);

        let ico = create_icon_image(icon.as_deref().unwrap_or("application-x-executable"));
        ico.set_pixel_size(48);
        ico.set_halign(Align::Center);
        ico.set_valign(Align::Center);
        ico.set_hexpand(true);
        ico.set_vexpand(true);
        ico.add_css_class("preview-placeholder");
        thumb.append(&ico);
        card.append(&thumb);

        // Focus on click anywhere on the card
        let addr_focus = win.address.clone();
        let click_g = gtk4::GestureClick::new();
        click_g.connect_pressed(move |_, _, _, _| {
            if let Err(e) = std::process::Command::new("hyprctl")
                .args(["dispatch", "focuswindow", &format!("address:{}", addr_focus)])
                .spawn()
            {
                log::warn!("Failed to focus window {}: {e}", addr_focus);
            }
        });
        card.add_controller(click_g);

        cards_row.append(&card);
    }

    cards_row
}

fn update_preview_content(
    ps:    &Rc<RefCell<PreviewState>>,
    class: &str,
    icon:  &Option<String>,
    btn:   &Button,
    config: &Config,
) {
    let handler = HyprlandHandler::new();
    let windows = handler.get_clients_for_class(class);
    if windows.is_empty() { return; }

    let dock_pos = ps.borrow().dock_position.clone();
    let (card_w, card_h, thumb_w, thumb_h) = if config.compact_preview {
        (204i32, 144i32, 200i32, 108i32)
    } else {
        (264i32, 196i32, 260i32, 160i32)
    };
    let content = build_preview_content(&windows, icon, &dock_pos, thumb_w, thumb_h);

    let mut s = ps.borrow_mut();

    let mut toplevel = btn.clone().upcast::<gtk4::Widget>();
    while let Some(parent) = toplevel.parent() { toplevel = parent; }
    let Ok(app_win) = toplevel.downcast::<ApplicationWindow>() else { return; };

    let display = app_win.upcast_ref::<gtk4::Widget>().display();
    let monitor = app_win.surface()
        .and_then(|s| display.monitor_at_surface(&s))
        .or_else(|| {
            display.monitors().item(0)
                .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
        });
    let Some(monitor) = monitor else { return; };

    s.win.set_child(Some(&content));
    s.active_class = class.to_string();

    let geo = monitor.geometry();
    let monitor_w = geo.width();
    let monitor_h = geo.height();
    let alloc = app_win.allocation();
    let dock_w = alloc.width();
    let dock_h = alloc.height();

    let (bx, by, bw, bh) = get_button_monitor_pos(btn, &dock_pos, config);

    s.win.set_margin(Edge::Left, 0);
    s.win.set_margin(Edge::Right, 0);
    s.win.set_margin(Edge::Top, 0);
    s.win.set_margin(Edge::Bottom, 0);

    let n = windows.len() as i32;

    const PANEL_PAD: i32 = 8;
    const CARD_GAP: i32 = 8;

    let mut margin_val = config.margin;
    if config.system_gap_used { margin_val = HyprlandHandler::new().get_gaps_out(); }
    let margin_bottom = config.margin_bottom + margin_val;
    let margin_top    = config.margin_top    + margin_val;
    let margin_left   = config.margin_left   + margin_val;
    let margin_right  = config.margin_right  + margin_val;

    match dock_pos.as_str() {
        "left" => {
            let panel_h = n * card_h + (n - 1).max(0) * CARD_GAP + 2 * PANEL_PAD;
            let max_top = (monitor_h - panel_h).max(0);
            s.win.set_margin(Edge::Top, (by + bh / 2 - panel_h / 2).clamp(0, max_top));
            s.win.set_margin(Edge::Left, margin_left + dock_w + 6);
        }
        "right" => {
            let panel_h = n * card_h + (n - 1).max(0) * CARD_GAP + 2 * PANEL_PAD;
            let max_top = (monitor_h - panel_h).max(0);
            s.win.set_margin(Edge::Top, (by + bh / 2 - panel_h / 2).clamp(0, max_top));
            s.win.set_margin(Edge::Right, margin_right + dock_w + 6);
        }
        "top" => {
            let panel_w = n * card_w + (n - 1).max(0) * CARD_GAP + 2 * PANEL_PAD;
            let max_left = (monitor_w - panel_w).max(0);
            s.win.set_margin(Edge::Left, (bx + bw / 2 - panel_w / 2).clamp(0, max_left));
            s.win.set_margin(Edge::Top, margin_top + dock_h + 6);
        }
        _ => {
            let panel_w = n * card_w + (n - 1).max(0) * CARD_GAP + 2 * PANEL_PAD;
            let max_left = (monitor_w - panel_w).max(0);
            s.win.set_margin(Edge::Left, (bx + bw / 2 - panel_w / 2).clamp(0, max_left));
            s.win.set_margin(Edge::Bottom, margin_bottom + dock_h + 6);
        }
    }

    s.win.set_monitor(Some(&monitor));

    if !s.visible {
        s.win.show();
        s.visible = true;
    }
    drop(s);

    spawn_screenshot_updates(windows, Rc::clone(ps), content);
}

/// Get the monitor-relative X, Y, W, H of a button widget.
fn get_button_monitor_pos(btn: &Button, dock_pos: &str, config: &Config) -> (i32, i32, i32, i32) {
    let mut toplevel = btn.clone().upcast::<gtk4::Widget>();
    while let Some(parent) = toplevel.parent() { toplevel = parent; }
    let app_win = match toplevel.downcast::<ApplicationWindow>() {
        Ok(w) => w,
        Err(_) => return (0, 0, 64, 64),
    };

    let display = app_win.upcast_ref::<gtk4::Widget>().display();
    let monitor = app_win.surface()
        .and_then(|s| display.monitor_at_surface(&s))
        .or_else(|| {
            display.monitors().item(0)
                .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
        });
    let monitor = match monitor {
        Some(m) => m,
        None => return (0, 0, 64, 64),
    };
    let geo = monitor.geometry();
    let monitor_w = geo.width();
    let monitor_h = geo.height();
    let alloc = app_win.allocation();
    let dock_w = alloc.width();
    let dock_h = alloc.height();

    let mut margin = config.margin;
    if config.system_gap_used { margin = HyprlandHandler::new().get_gaps_out(); }
    let margin_bottom = config.margin_bottom + margin;
    let margin_top = config.margin_top + margin;
    let margin_left = config.margin_left + margin;
    let margin_right = config.margin_right + margin;

    let (tx, ty) = match dock_pos {
        "top" => {
            let x = if config.full_screen { margin_left } else { (monitor_w - dock_w) / 2 };
            (x, margin_top)
        }
        "left" => {
            let y = if config.full_screen { margin_top } else { (monitor_h - dock_h) / 2 };
            (margin_left, y)
        }
        "right" => {
            let y = if config.full_screen { margin_top } else { (monitor_h - dock_h) / 2 };
            (monitor_w - margin_right - dock_w, y)
        }
        _ => {
            let x = if config.full_screen { margin_left } else { (monitor_w - dock_w) / 2 };
            (x, monitor_h - margin_bottom - dock_h)
        }
    };

    let (wx, wy) = btn.translate_coordinates(&app_win, 0.0, 0.0).unwrap_or((0.0, 0.0));
    let btn_alloc = btn.allocation();
    (tx + wx as i32, ty + wy as i32, btn_alloc.width(), btn_alloc.height())
}

fn spawn_screenshot_updates(
    windows:       Vec<HyprClient>,
    preview_state: Rc<RefCell<PreviewState>>,
    content:       Box,
) {
    let windows = Rc::new(windows);
    let (tx, rx) = mpsc::channel::<(usize, String)>();
    let rx = Rc::new(RefCell::new(rx));
    let content_ptr = content.clone();
    let pending = Arc::new(std::sync::Mutex::new(vec![false; windows.len()]));

    spawn_pending_captures(&windows, &tx, &pending);

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let s = preview_state.borrow();
        if !s.visible { return glib::ControlFlow::Break; }
        if let Some(child) = s.win.child() {
            if child != content_ptr.clone().upcast::<gtk4::Widget>() {
                return glib::ControlFlow::Break;
            }
        } else {
            return glib::ControlFlow::Break;
        }
        drop(s);

        loop {
            match rx.borrow().try_recv() {
                Ok((idx, path)) => {
                    let mut card_w = content_ptr.first_child();
                    let mut i = 0;
                    while let Some(w) = card_w {
                        if i == idx {
                            // Card is a Box: first child = header, second child = thumb.
                            if let Ok(card) = w.clone().downcast::<Box>() {
                                if let Some(thumb_widget) = card.first_child()
                                    .and_then(|h| h.next_sibling())
                                {
                                    if let Ok(thumb) = thumb_widget.downcast::<Box>() {
                                        if let Some(old) = thumb.first_child() { thumb.remove(&old); }
                                        let pic = gtk4::Picture::builder()
                                            .file(&gio::File::for_path(&path))
                                            .halign(Align::Fill)
                                            .valign(Align::Fill)
                                            .hexpand(true)
                                            .vexpand(true)
                                            .can_shrink(true)
                                            .css_classes(vec!["win-thumbnail".to_string()])
                                            .build();
                                        pic.set_content_fit(gtk4::ContentFit::Cover);
                                        thumb.append(&pic);
                                    }
                                }
                            }
                            break;
                        }
                        i += 1;
                        card_w = w.next_sibling();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }

        spawn_pending_captures(&windows, &tx, &pending);

        glib::ControlFlow::Continue
    });
}

const MAX_CONCURRENT_CAPTURES: usize = 4;

fn spawn_pending_captures(
    windows: &Rc<Vec<HyprClient>>,
    tx:      &mpsc::Sender<(usize, String)>,
    pending: &Arc<std::sync::Mutex<Vec<bool>>>,
) {
    let mut pend = pending.lock().unwrap();
    let in_progress = pend.iter().filter(|&&p| p).count();
    let mut slots = MAX_CONCURRENT_CAPTURES.saturating_sub(in_progress);

    for (idx, win) in windows.iter().enumerate() {
        if slots == 0 { break; }
        if !pend[idx] {
            pend[idx] = true;
            slots -= 1;
            let tx2   = tx.clone();
            let addr  = win.address.clone();
            let sid   = win.stable_id.clone();
            let at    = win.at;
            let size  = win.size;
            let pend2 = Arc::clone(pending);
            std::thread::spawn(move || {
                if let Some(path) = capture_window_screenshot(&addr, &sid, at, size) {
                    let _ = tx2.send((idx, path));
                }
                if let Ok(mut p) = pend2.lock() { p[idx] = false; }
            });
        }
    }
}

// ── Helper trait ─────────────────────────────────────────────────────────────
trait CellBorrowPeek {
    fn borrow_peek(&self) -> bool;
}
impl CellBorrowPeek for Cell<Option<glib::SourceId>> {
    fn borrow_peek(&self) -> bool {
        let val = self.take();
        let has = val.is_some();
        self.set(val);
        has
    }
}

fn create_icon_image(icon_name: &str) -> Image {
    // Absolute or relative file path — load directly.
    if icon_name.contains('/') {
        let p = std::path::Path::new(icon_name);
        if p.exists() {
            return Image::from_gicon(&gio::FileIcon::new(&gio::File::for_path(p)));
        }
    }

    // Strip image extensions that occasionally appear in Icon= fields
    // (e.g. "myapp.png"). Never strip dots that are part of the name itself
    // (e.g. "com.github.rafostar.Clapper" must stay intact).
    let base = icon_name
        .strip_suffix(".png")
        .or_else(|| icon_name.strip_suffix(".svg"))
        .or_else(|| icon_name.strip_suffix(".xpm"))
        .unwrap_or(icon_name);

    // Ask GTK's icon theme first — it searches the active theme, all parent
    // themes (hicolor, Adwaita, breeze), and every path in XDG_DATA_DIRS
    // including Flatpak exports. No auto-generated intermediate names.
    // The "application-x-executable" fallback ensures we never render blank.
    if let Some(display) = gtk4::gdk::Display::default() {
        let theme = gtk4::IconTheme::for_display(&display);
        if theme.has_icon(base) {
            return Image::from_icon_name(base);
        }
    }

    // GTK didn't find it — search icon files directly across all data dirs.
    // This covers pixmaps and hicolor sizes not indexed in the theme cache.
    if let Some(path) = find_icon_file(base) {
        return Image::from_gicon(&gio::FileIcon::new(&gio::File::for_path(&path)));
    }

    Image::from_icon_name("application-x-executable")
}

/// Search for a matching icon file across every XDG data directory, checking
/// all common hicolor sizes, scalable, and pixmaps in priority order.
fn find_icon_file(name: &str) -> Option<std::path::PathBuf> {
    // Build the list of data directories to search (XDG_DATA_DIRS + HOME dirs).
    let xdg: Vec<std::path::PathBuf> = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .map(std::path::PathBuf::from)
        .collect();

    let mut dirs = xdg;
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(std::path::PathBuf::from(&home).join(".local/share"));
        dirs.push(std::path::PathBuf::from(&home).join(".icons"));
    }

    // Preferred sizes in lookup order (largest first for best quality).
    let sizes = ["scalable", "512x512", "256x256", "128x128", "96x96",
                 "64x64", "48x48", "32x32", "24x24", "22x22", "16x16"];

    for dir in &dirs {
        // 1. hicolor theme at all sizes
        for size in &sizes {
            for ext in &["svg", "png", "xpm"] {
                let p = dir.join(format!("icons/hicolor/{}/apps/{}.{}", size, name, ext));
                if p.exists() { return Some(p); }
            }
        }
        // 2. Any installed icon theme (catches themes GTK may not have indexed)
        for ext in &["svg", "png"] {
            let p = dir.join(format!("icons/{}/{}", name, ext));
            if p.exists() { return Some(p); }
        }
        // 3. pixmaps (legacy apps that install icons here)
        for ext in &["png", "svg", "xpm"] {
            let p = dir.join(format!("pixmaps/{}.{}", name, ext));
            if p.exists() { return Some(p); }
        }
    }
    None
}
