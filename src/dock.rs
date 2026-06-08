use gio;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Image, Orientation,
    Label, EventControllerMotion,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use crate::config::Config;
use crate::hyprland_handler::{HyprlandHandler, HyprClient, capture_window_screenshot};
use crate::app_info::AppInfo;
use std::rc::Rc;
use std::cell::{RefCell, Cell};
use std::sync::mpsc;

// ── Preview window state shared across all buttons ──────────────────────────
struct PreviewState {
    /// The dedicated layer-shell preview window (never destroyed, just shown/hidden)
    win:              ApplicationWindow,
    /// Which app class is currently shown (empty = hidden)
    active_class:     String,
    /// Whether the preview window is visible right now
    visible:          bool,
    /// Which screen edge the dock is on ("bottom", "top", "left", "right")
    dock_position:    String,
    /// Smart hide timer of the main dock window
    smart_hide_timer: Rc<Cell<Option<glib::SourceId>>>,
    /// Whether smart view is enabled
    smart_view:       bool,
    /// Auto hide delay of the main dock window
    auto_hide_delay:  u64,
    /// Edge margin fallback
    edge_margin:      i32,
    /// The main dock window
    dock_win:         ApplicationWindow,
}

pub struct Dock {
    pub window:           ApplicationWindow,
    pub detect_window:    Option<ApplicationWindow>,
    pub box_container:    Box,
    pub config:           Rc<RefCell<Config>>,
    pub hyprland:         HyprlandHandler,
    pub smart_hide_timer: Rc<Cell<Option<glib::SourceId>>>,
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
        window.set_layer(Layer::Top);
        window.set_namespace(Some("rust-dock"));

        let cfg = config.borrow();
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
        window.set_exclusive_zone(-1);

        let mut margin = cfg.margin;
        if cfg.system_gap_used { margin = HyprlandHandler::new().get_gaps_out(); }
        window.set_margin(Edge::Bottom, cfg.margin_bottom + margin);
        window.set_margin(Edge::Top,    cfg.margin_top    + margin);
        window.set_margin(Edge::Left,   cfg.margin_left   + margin);
        window.set_margin(Edge::Right,  cfg.margin_right  + margin);

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
        box_container.set_margin_start(6);
        box_container.set_margin_end(6);
        box_container.set_margin_top(4);
        box_container.set_margin_bottom(4);
        window.set_child(Some(&box_container));

        if let Some(monitor_name) = &cfg.output {
            let display = gtk4::gdk::Display::default().expect("no display");
            let monitors = display.monitors();
            for i in 0..monitors.n_items() {
                if let Some(m) = monitors.item(i).and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
                    if m.connector().map(|c| c.to_string()).as_deref() == Some(monitor_name) {
                        window.set_monitor(Some(&m)); break;
                    }
                }
            }
        }

        let smart_view       = cfg.smart_view;
        let auto_hide_delay  = cfg.auto_hide_delay as u64;
        let dock_position    = cfg.position.clone();
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
                let display = gtk4::gdk::Display::default().expect("no display");
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

            let motion_det  = EventControllerMotion::new();
            let st_f        = Rc::clone(&smart_hide_timer);
            let wc_f        = window.clone();
            motion_det.connect_enter(move |_, _, _| {
                if let Some(id) = st_f.take() { id.remove(); }
                wc_f.set_layer(Layer::Top);
            });
            let st_u  = Rc::clone(&smart_hide_timer);
            let wc_u  = window.clone();
            motion_det.connect_leave(move |_| {
                if let Some(id) = st_u.take() { id.remove(); }
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
                if let Some(id) = st_f2.take() { id.remove(); }
                wc1.set_layer(Layer::Top);
            }
        });
        let wc2  = window.clone();
        let st_u2 = Rc::clone(&smart_hide_timer);
        motion.connect_leave(move |_| {
            if smart_view {
                if let Some(id) = st_u2.take() { id.remove(); }
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
        // Layer::Overlay so it floats above everything; it stays alive forever,
        // we just show/hide it and swap its content.
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

        // Anchor the preview window to the same edge as the dock
        // We also must anchor to Top/Left so that margins can push it into position.
        match dock_position.as_str() {
            "top"   => { preview_win.set_anchor(Edge::Top, true); preview_win.set_anchor(Edge::Left, true); }
            "left"  => { preview_win.set_anchor(Edge::Left, true); preview_win.set_anchor(Edge::Top, true); }
            "right" => { preview_win.set_anchor(Edge::Right, true); preview_win.set_anchor(Edge::Top, true); }
            _       => { preview_win.set_anchor(Edge::Bottom, true); preview_win.set_anchor(Edge::Left, true); }
        }

        if let Some(monitor_name) = &cfg.output {
            let display = gtk4::gdk::Display::default().expect("no display");
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
        preview_win.add_css_class("dock-preview-window");
        // Don't show preview_win yet — hide until needed

        drop(cfg);

        window.show();

        let dock = Self {
            window,
            detect_window,
            box_container,
            config,
            hyprland: HyprlandHandler::new(),
            smart_hide_timer,
        };
        dock.refresh_with_preview(preview_win, 60, dock_position);
        dock
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
        // On hyprland events we can't pass the preview_win — just clear & rebuild
        // by calling the real method. We keep a stored reference to preview_win
        // by looking it up via the application window list.
        let app = self.window.application().expect("no application");
        let dock_pos = self.config.borrow().position.clone();
        // Find the dock-preview window among all app windows
        for win in app.windows() {
            if win.title().as_deref() == Some("dock-preview") {
                if let Ok(pw) = win.downcast::<ApplicationWindow>() {
                    // Recover the edge_margin and dock_position from the existing PreviewState
                    // by using the dock-preview window's current margins as a fallback of 60px
                    self.refresh_with_preview(pw, 60, dock_pos);
                    return;
                }
            }
        }
    }

    fn refresh_with_preview(&self, preview_win: ApplicationWindow, edge_margin: i32, dock_position: String) {
        // Clear existing buttons
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

        let mut app_counts: std::collections::HashMap<String, usize> = Default::default();
        let mut running_ordered: Vec<AppInfo> = Vec::new();

        for client in self.hyprland.get_clients() {
            let class = client.class.clone();
            if class.is_empty() { continue; }
            let app = AppInfo::find_by_class(&class).unwrap_or_else(|| AppInfo {
                id: class.clone(),
                name: if client.title.is_empty() { class.clone() } else { client.title.clone() },
                icon: Some("application-x-executable".to_string()),
                exec: "".to_string(),
            });
            if !app_counts.contains_key(&app.id) { running_ordered.push(app.clone()); }
            *app_counts.entry(app.id.clone()).or_insert(0) += 1;
        }

        let pinned_apps = self.config.borrow().pinned_apps.clone();

        // ── Shared preview state ──────────────────────────────────────────
        // One PreviewState instance for the whole dock. All buttons share it.
        let preview_state: Rc<RefCell<PreviewState>> = Rc::new(RefCell::new(PreviewState {
            win:              preview_win.clone(),
            active_class:     String::new(),
            visible:          false,
            edge_margin,
            dock_position:    dock_position.clone(),
            smart_hide_timer: Rc::clone(&self.smart_hide_timer),
            smart_view:       self.config.borrow().smart_view,
            auto_hide_delay:  self.config.borrow().auto_hide_delay as u64,
            dock_win:         self.window.clone(),
        }));
        let show_timer: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
        let hide_timer: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));

        // Add motion controller to the preview window itself so that
        // moving into the preview cancels the hide timer.
        let ht_preview  = Rc::clone(&hide_timer);
        let st_preview  = Rc::clone(&show_timer);
        let ps_preview  = Rc::clone(&preview_state);
        let pv_motion   = EventControllerMotion::new();
        pv_motion.connect_enter(move |_, _, _| {
            if let Some(id) = ht_preview.take() { id.remove(); }
            if let Some(id) = st_preview.take() { id.remove(); }

            let ps = ps_preview.borrow();
            if ps.smart_view {
                if let Some(id) = ps.smart_hide_timer.take() { id.remove(); }
                ps.dock_win.set_layer(Layer::Top);
            }
        });
        let ps_leave    = Rc::clone(&preview_state);
        let ht_leave    = Rc::clone(&hide_timer);
        pv_motion.connect_leave(move |_| {
            let ps  = Rc::clone(&ps_leave);
            let ht  = Rc::clone(&ht_leave);
            let id = glib::timeout_add_local(std::time::Duration::from_millis(350), move || {
                let mut s = ps.borrow_mut();
                s.win.hide();
                s.visible = false;
                s.active_class = String::new();
                ht.set(None);

                if s.smart_view {
                    if let Some(id) = s.smart_hide_timer.take() { id.remove(); }
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
        // Ensure preview window does not steal input if possible or handle pointer correctly
        // Layer-shell windows on Overlay might block if they have input focus.
        // Try to set it to not focusable.
        preview_win.set_focusable(false);

        for app_id in &pinned_apps {
            if let Some(app) = AppInfo::find_by_id(app_id) {
                let instances = app_counts.remove(&app.id).unwrap_or(0);
                self.add_app_button(&app, instances, true,
                    Rc::clone(&preview_state),
                    Rc::clone(&show_timer),
                    Rc::clone(&hide_timer),
                    Rc::clone(&self.config),
                );
            }
        }
        for app in running_ordered {
            if !pinned_apps.contains(&app.id) {
                if let Some(instances) = app_counts.get(&app.id) {
                    self.add_app_button(&app, *instances, false,
                        Rc::clone(&preview_state),
                        Rc::clone(&show_timer),
                        Rc::clone(&hide_timer),
                        Rc::clone(&self.config),
                    );
                }
            }
        }

        self.box_container.queue_resize();
        self.window.queue_resize();
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
    ) {
        let btn = Button::builder()
            .css_classes(vec![if pinned { "pinned" } else { "running" }.to_string()])
            .build();

        let vbox = Box::builder().orientation(Orientation::Vertical).spacing(2).build();
        if let Some(icon_name) = &app.icon {
            let img = Image::from_icon_name(icon_name);
            img.set_pixel_size(self.config.borrow().icon_size);
            vbox.append(&img);
        } else {
            vbox.append(&Label::new(Some(&app.name)));
        }
        if instances > 0 {
            vbox.append(&Label::builder()
                .label(&"•".repeat(instances.min(5)))
                .css_classes(vec!["indicator".to_string()])
                .build());
        }
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
                // Always cancel a pending hide
                if let Some(id) = ht_e.take() { id.remove(); }

                let state_visible;
                let state_class;
                {
                    let s = ps_e.borrow();
                    state_visible = s.visible;
                    state_class   = s.active_class.clone();
                }

                if state_visible {
                    if state_class == class_e {
                        return; // same app, nothing to do
                    }
                    // Different app: fast transition without hide/show cycle
                    if let Some(id) = st_e.take() { id.remove(); }

                    let ps  = Rc::clone(&ps_e);
                    let cls = class_e.clone();
                    let icn = icon_e.clone();
                    let btn = btn_e.clone();
                    let cfg_c = Rc::clone(&config_e);
                    let id = glib::timeout_add_local(
                        std::time::Duration::from_millis(move_delay),
                        move || {
                            update_preview_content(&ps, &cls, &icn, &btn, &cfg_c.borrow());
                            glib::ControlFlow::Break
                        }
                    );
                    st_e.set(Some(id));
                } else {
                    // Not visible: schedule show after show_delay
                    if st_e.borrow_peek() { return; } // already pending

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
                if let Some(id) = st_l.take() { id.remove(); }

                let visible = ps_l.borrow().visible;
                if !visible { return; }

                let ps  = Rc::clone(&ps_l);
                let ht  = Rc::clone(&ht_l);
                let id = glib::timeout_add_local(
                    std::time::Duration::from_millis(hide_delay),
                    move || {
                        let mut s = ps.borrow_mut();
                        s.win.hide();
                        s.visible = false;
                        s.active_class = String::new();
                        ht.set(None);
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
                .has_arrow(true)
                .build();
            menu_pop.add_css_class("context-popover");
            menu_pop.set_parent(&btn);

            let mp_d = menu_pop.clone();
            btn.connect_destroy(move |_| { mp_d.unparent(); });

            let app_id_m    = app.id.clone();
            let app_class_m = app.id.clone();
            let config_m    = Rc::clone(&config);
            let ps_m        = Rc::clone(&preview_state);
            let st_m        = Rc::clone(&show_timer);
            let ht_m        = Rc::clone(&hide_timer);

            menu_pop.connect_show(move |pop| {
                // Hide preview when context menu opens
                if let Some(id) = st_m.take() { id.remove(); }
                if let Some(id) = ht_m.take() { id.remove(); }
                {
                    let mut s = ps_m.borrow_mut();
                    s.win.hide();
                    s.visible = false;
                    s.active_class = String::new();
                }

                if let Some(old) = pop.child() { drop(old); pop.set_child(gtk4::Widget::NONE); }

                let row = Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(6)
                    .margin_top(6).margin_bottom(6)
                    .margin_start(8).margin_end(8)
                    .build();

                let pin_lbl = if pinned { "⊘  Unpin" } else { "📌  Pin" };
                let pin_btn = Button::builder().label(pin_lbl)
                    .css_classes(vec!["pop-action-btn".to_string()]).build();
                let cfg_c = Rc::clone(&config_m);
                let id_c  = app_id_m.clone();
                let pop_r = pop.clone();
                pin_btn.connect_clicked(move |_| {
                    let mut c = cfg_c.borrow_mut();
                    if pinned { c.unpin_app(&id_c); } else { c.pin_app(&id_c); }
                    pop_r.popdown();
                });
                row.append(&pin_btn);

                if instances > 0 {
                    row.append(&gtk4::Separator::new(Orientation::Vertical));
                    let close_btn = Button::builder().label("✕  Close All")
                        .css_classes(vec!["pop-action-btn".to_string(), "pop-close-btn".to_string()])
                        .build();
                    let class_c = app_class_m.clone();
                    let pop_r2  = pop.clone();
                    close_btn.connect_clicked(move |_| {
                        let _ = std::process::Command::new("hyprctl")
                            .args(["dispatch", "closewindow", &format!("^{}$", class_c)])
                            .spawn();
                        pop_r2.popdown();
                    });
                    row.append(&close_btn);
                }
                pop.set_child(Some(&row));
            });

            let rclick   = gtk4::GestureClick::new();
            rclick.set_button(3);
            let mp_click = menu_pop.clone();
            rclick.connect_pressed(move |_, _, _, _| { mp_click.popup(); });
            btn.add_controller(rclick);
        }

        let app_clone = app.clone();
        btn.connect_clicked(move |_| { app_clone.launch(); });

        self.box_container.append(&btn);
    }
}

// ── Preview window content management ───────────────────────────────────────

/// Build/update the preview window content for `class`, then show the window.
/// The window is never destroyed — we just swap its child widget.
/// Build/update the preview window content for `class`, then show the window.
/// The window is never destroyed — we just swap its child widget.
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
    let content = build_preview_content(&windows, icon, &dock_pos);

    // Position the preview window near the button.
    let (bx, by, bw, bh) = get_button_monitor_pos(btn, &dock_pos, config);

    {
        let mut s = ps.borrow_mut();
        s.win.set_child(Some(&content));
        s.active_class = class.to_string();

        let mut toplevel = btn.clone().upcast::<gtk4::Widget>();
        while let Some(parent) = toplevel.parent() {
            toplevel = parent;
        }
        if let Ok(app_win) = toplevel.downcast::<ApplicationWindow>() {
            let display = app_win.upcast_ref::<gtk4::Widget>().display();
            let monitor = if let Some(surface) = app_win.surface() {
                display.monitor_at_surface(&surface)
            } else {
                None
            };
            if let Some(ref m) = monitor {
                s.win.set_monitor(Some(m));
            }

            let monitor = monitor.unwrap_or_else(|| {
                display.monitors().item(0)
                    .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
                    .expect("no monitor found")
            });
            let geo = monitor.geometry();
            let monitor_w = geo.width();
            let monitor_h = geo.height();
            let alloc = app_win.allocation();
            let dock_w = alloc.width();
            let dock_h = alloc.height();

            // Reset all margins first to prevent residual margins from different layouts
            s.win.set_margin(Edge::Left, 0);
            s.win.set_margin(Edge::Right, 0);
            s.win.set_margin(Edge::Top, 0);
            s.win.set_margin(Edge::Bottom, 0);

            match dock_pos.as_str() {
                "left" => {
                    let card_h = 130_i32; // Vertical preview height per card
                    let preview_h = (windows.len() as i32) * card_h + 16;
                    let top_margin = (by + bh / 2 - preview_h / 2).max(0);
                    s.win.set_margin(Edge::Top, top_margin);
                    
                    let left_margin = bx + dock_w + 5;
                    s.win.set_margin(Edge::Left, left_margin);
                }
                "right" => {
                    let card_h = 130_i32;
                    let preview_h = (windows.len() as i32) * card_h + 16;
                    let top_margin = (by + bh / 2 - preview_h / 2).max(0);
                    s.win.set_margin(Edge::Top, top_margin);
                    
                    let right_margin = monitor_w - bx + 5;
                    s.win.set_margin(Edge::Right, right_margin);
                }
                "top" => {
                    let card_w = 180_i32;
                    let preview_w = (windows.len() as i32) * card_w + 16;
                    let left_margin = (bx + bw / 2 - preview_w / 2).max(0);
                    s.win.set_margin(Edge::Left, left_margin);
                    
                    let top_margin = by + dock_h + 5;
                    s.win.set_margin(Edge::Top, top_margin);
                }
                _ => { // "bottom"
                    let card_w = 180_i32;
                    let preview_w = (windows.len() as i32) * card_w + 16;
                    let left_margin = (bx + bw / 2 - preview_w / 2).max(0);
                    s.win.set_margin(Edge::Left, left_margin);
                    
                    let bottom_margin = monitor_h - by + 5;
                    s.win.set_margin(Edge::Bottom, bottom_margin);
                }
            }
        }

        if !s.visible {
            s.win.show();
            s.visible = true;
        }
    }

    // Kick off async screenshot capture
    let sp = Rc::clone(ps);
    spawn_screenshot_updates(windows, sp, content);
}

/// Get the monitor-relative X, Y, W, H of a button widget.
fn get_button_monitor_pos(btn: &Button, dock_pos: &str, config: &Config) -> (i32, i32, i32, i32) {
    let mut toplevel = btn.clone().upcast::<gtk4::Widget>();
    // Check if the button itself is not null and has a parent
    while let Some(parent) = toplevel.parent() {
        toplevel = parent;
    }
    
    let app_win = match toplevel.downcast::<ApplicationWindow>() {
        Ok(w) => w,
        Err(_) => {
            println!("[DEBUG] Failed to find ApplicationWindow for button");
            return (0, 0, 64, 64);
        }
    };

    let display = app_win.upcast_ref::<gtk4::Widget>().display();
    let monitor = if let Some(surface) = app_win.surface() {
        display.monitor_at_surface(&surface).unwrap_or_else(|| {
            display.monitors().item(0)
                .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
                .expect("no monitor found")
        })
    } else {
        display.monitors().item(0)
            .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
            .expect("no monitor found")
    };

    let geo = monitor.geometry();
    let monitor_w = geo.width();
    let monitor_h = geo.height();

    let alloc = app_win.allocation();
    let dock_w = alloc.width();
    let dock_h = alloc.height();

    let mut margin = config.margin;
    if config.system_gap_used {
        margin = HyprlandHandler::new().get_gaps_out();
    }
    let margin_bottom = config.margin_bottom + margin;
    let margin_top = config.margin_top + margin;
    let margin_left = config.margin_left + margin;
    let margin_right = config.margin_right + margin;

    let (tx, ty) = match dock_pos {
        "top" => {
            let x = if config.full_screen {
                margin_left
            } else {
                (monitor_w - dock_w) / 2
            };
            let y = margin_top;
            (x, y)
        }
        "left" => {
            let x = margin_left;
            let y = if config.full_screen {
                margin_top
            } else {
                (monitor_h - dock_h) / 2
            };
            (x, y)
        }
        "right" => {
            let x = monitor_w - margin_right - dock_w;
            let y = if config.full_screen {
                margin_top
            } else {
                (monitor_h - dock_h) / 2
            };
            (x, y)
        }
        _ => { // "bottom"
            let x = if config.full_screen {
                margin_left
            } else {
                (monitor_w - dock_w) / 2
            };
            let y = monitor_h - margin_bottom - dock_h;
            (x, y)
        }
    };

    let (wx, wy) = btn.translate_coordinates(&app_win, 0.0, 0.0)
        .unwrap_or((0.0, 0.0));
    let btn_alloc = btn.allocation();

    (
        tx + wx as i32,
        ty + wy as i32,
        btn_alloc.width(),
        btn_alloc.height(),
    )
}

fn build_preview_content(
    windows: &[HyprClient],
    icon:    &Option<String>,
    dock_pos: &str,
) -> Box {
    let orientation = match dock_pos {
        "left" | "right" => Orientation::Vertical,
        _ => Orientation::Horizontal,
    };
    let cards_row = Box::builder()
        .orientation(orientation)
        .spacing(8)
        .margin_top(8).margin_bottom(8)
        .margin_start(8).margin_end(8)
        .css_classes(vec!["preview-row".to_string()])
        .build();

    for win in windows {
        let card = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .css_classes(vec!["win-card".to_string()])
            .build();
        card.set_size_request(164, -1);

        // Title bar
        let title_bar = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_top(5).margin_bottom(3)
            .margin_start(7).margin_end(5)
            .build();

        let title_str = if win.title.chars().count() > 22 {
            format!("{}…", win.title.chars().take(22).collect::<String>())
        } else { win.title.clone() };

        title_bar.append(&Label::builder()
            .label(&title_str)
            .hexpand(true).xalign(0.0)
            .css_classes(vec!["win-title".to_string()])
            .build());

        let close_x = Button::builder().label("×")
            .css_classes(vec!["win-close-btn".to_string()]).build();
        let addr_close = win.address.clone();
        close_x.connect_clicked(move |_| {
            let _ = std::process::Command::new("hyprctl")
                .args(["dispatch", "closewindow", &format!("address:{}", addr_close)])
                .spawn();
        });
        title_bar.append(&close_x);
        card.append(&title_bar);

        // Thumbnail area (icon placeholder, replaced by screenshot)
        let thumb = Box::builder()
            .orientation(Orientation::Vertical)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .css_classes(vec!["win-thumb-box".to_string()])
            .build();
        thumb.set_size_request(156, 88);

        let ico = Image::from_icon_name(icon.as_deref().unwrap_or("application-x-executable"));
        ico.set_pixel_size(48);
        ico.set_halign(gtk4::Align::Center);
        ico.set_valign(gtk4::Align::Center);
        ico.set_hexpand(true);
        ico.set_vexpand(true);
        ico.add_css_class("preview-placeholder");
        thumb.append(&ico);
        card.append(&thumb);

        // Focus on click
        let addr_focus = win.address.clone();
        let click_g = gtk4::GestureClick::new();
        click_g.connect_pressed(move |_, _, _, _| {
            let _ = std::process::Command::new("hyprctl")
                .args(["dispatch", "focuswindow", &format!("address:{}", addr_focus)])
                .spawn();
        });
        card.add_controller(click_g);

        cards_row.append(&card);
    }

    cards_row
}

/// Capture screenshots asynchronously and fill in thumbnails.
fn spawn_screenshot_updates(
    windows:       Vec<HyprClient>,
    preview_state: Rc<RefCell<PreviewState>>,
    content:       Box,
) {
    let (tx, rx) = mpsc::channel::<(usize, String)>();

    for (idx, win) in windows.iter().enumerate() {
        let tx2  = tx.clone();
        let addr = win.address.clone();
        let at   = win.at;
        let size = win.size;
        std::thread::spawn(move || {
            if let Some(path) = capture_window_screenshot(&addr, at, size) {
                let _ = tx2.send((idx, path));
            }
        });
    }
    drop(tx);

    let rx = Rc::new(RefCell::new(rx));
    let content_ptr = content.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        let win = {
            let s = preview_state.borrow();
            if !s.visible { return glib::ControlFlow::Break; }
            
            // If the window child has changed, this update loop is stale
            if let Some(child) = s.win.child() {
                if child != content_ptr.clone().upcast::<gtk4::Widget>() {
                    return glib::ControlFlow::Break;
                }
            } else {
                return glib::ControlFlow::Break;
            }

            s.win.clone()
        };

        loop {
            match rx.borrow().try_recv() {
                Ok((idx, path)) => {
                    // cards_row → card[idx] → title_bar(0), thumb_box(1)
                    let mut card_w = content_ptr.first_child();
                    let mut i = 0;
                    while let Some(w) = card_w {
                        if i == idx {
                            if let Ok(card) = w.downcast::<Box>() {
                                let mut child = card.first_child();
                                let mut count = 0;
                                while let Some(c) = child {
                                    if count == 1 {
                                        if let Ok(thumb) = c.clone().downcast::<Box>() {
                                            if let Some(old) = thumb.first_child() {
                                                thumb.remove(&old);
                                            }
                                            let f = gio::File::for_path(&path);
                                            if let Ok(tex) = gtk4::gdk::Texture::from_file(&f) {
                                                let pic = gtk4::Picture::for_paintable(&tex);
                                                pic.set_size_request(156, 88);
                                                pic.set_halign(gtk4::Align::Fill);
                                                pic.set_valign(gtk4::Align::Fill);
                                                pic.set_hexpand(true);
                                                pic.set_vexpand(true);
                                                pic.add_css_class("win-thumbnail");
                                                thumb.append(&pic);
                                            }
                                        }
                                        break;
                                    }
                                    count += 1;
                                    child = c.next_sibling();
                                }
                            }
                            break;
                        }
                        i += 1;
                        card_w = w.next_sibling();
                    }
                }
                Err(mpsc::TryRecvError::Empty)        => break,
                Err(mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
        glib::ControlFlow::Continue
    });
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
