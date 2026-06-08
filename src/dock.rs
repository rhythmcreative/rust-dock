use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Image, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use crate::config::Config;
use crate::hyprland_handler::HyprlandHandler;
use crate::app_info::AppInfo;

pub struct Dock {
    pub window: ApplicationWindow,
    pub box_container: Box,
    pub config: Config,
    pub hyprland: HyprlandHandler,
}

impl Dock {
    pub fn new(app: &Application, config: Config) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("rust-dock")
            .build();

        // Initialize Layer Shell
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace(Some("rust-dock"));

        // Set position
        match config.position.as_str() {
            "top" => {
                window.set_anchor(Edge::Top, true);
                if config.full_screen {
                    window.set_anchor(Edge::Left, true);
                    window.set_anchor(Edge::Right, true);
                }
            }
            "left" => {
                window.set_anchor(Edge::Left, true);
                if config.full_screen {
                    window.set_anchor(Edge::Top, true);
                    window.set_anchor(Edge::Bottom, true);
                }
            }
            "right" => {
                window.set_anchor(Edge::Right, true);
                if config.full_screen {
                    window.set_anchor(Edge::Top, true);
                    window.set_anchor(Edge::Bottom, true);
                }
            }
            _ => { // default bottom
                window.set_anchor(Edge::Bottom, true);
                if config.full_screen {
                    window.set_anchor(Edge::Left, true);
                    window.set_anchor(Edge::Right, true);
                }
            }
        }

        if config.exclusive_zone {
            window.set_exclusive_zone(config.icon_size + 20);
        }

        // Margins
        window.set_margin(Edge::Bottom, config.margin_bottom);
        window.set_margin(Edge::Top, config.margin_top);
        window.set_margin(Edge::Left, config.margin_left);
        window.set_margin(Edge::Right, config.margin_right);

        let orientation = if config.position == "left" || config.position == "right" {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };

        let box_container = Box::builder()
            .orientation(orientation)
            .spacing(4)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .css_classes(vec!["dock-container".to_string()])
            .build();
        
        box_container.set_margin_start(0);
        box_container.set_margin_end(0);
        box_container.set_margin_top(0);
        box_container.set_margin_bottom(0);

        window.set_child(Some(&box_container));

        // Set Monitor
        if let Some(monitor_name) = &config.output {
            let display = gtk4::gdk::Display::default().expect("Could not get display");
            let monitors = display.monitors();
            for i in 0..monitors.n_items() {
                if let Some(monitor) = monitors.item(i).and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
                    if monitor.connector().map(|c| c.to_string()).as_deref() == Some(monitor_name) {
                        window.set_monitor(Some(&monitor));
                        break;
                    }
                }
            }
        }

        window.show();

        let hyprland = HyprlandHandler::new();

        let dock = Self {
            window,
            box_container,
            config,
            hyprland,
        };

        dock.refresh();

        dock
    }

    pub fn toggle_visibility(&self) {
        if self.window.is_visible() {
            self.window.hide();
        } else {
            self.window.show();
        }
    }

    pub fn set_dock_visible(&self, visible: bool) {
        if visible {
            self.window.show();
        } else {
            self.window.hide();
        }
    }

    pub fn refresh(&self) {
        // Clear existing children
        while let Some(child) = self.box_container.first_child() {
            self.box_container.remove(&child);
        }

        // Add launcher button
        if !self.config.no_launcher {
            let launcher_btn = Button::builder()
                .icon_name("start-here-symbolic")
                .css_classes(vec!["launcher-btn".to_string()])
                .build();
            
            let cmd = self.config.launcher_command.clone();
            launcher_btn.connect_clicked(move |_| {
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
            });
            self.box_container.append(&launcher_btn);
        }

        // Add pinned apps
        for app_id in &self.config.pinned_apps {
            if let Some(app) = AppInfo::find_by_id(app_id) {
                self.add_app_button(&app, true);
            }
        }

        // Add separator if there are pinned apps and clients
        // TODO: Implement separator styling

        // Add running apps (clients)
        let clients = self.hyprland.get_clients();
        let mut added_classes = std::collections::HashSet::new();
        
        for client in clients {
            let class = client.class.clone();
            if !added_classes.contains(&class) {
                 if let Some(app) = AppInfo::find_by_class(&class) {
                     // Check if not already pinned
                     if !self.config.pinned_apps.contains(&app.id) {
                        self.add_app_button(&app, false);
                        added_classes.insert(class);
                     }
                 }
            }
        }
    }

    fn add_app_button(&self, app: &AppInfo, pinned: bool) {
        let btn = Button::builder()
            .css_classes(vec![if pinned { "pinned" } else { "running" }.to_string()])
            .build();
        
        if let Some(icon_name) = &app.icon {
            let img = Image::from_icon_name(icon_name);
            img.set_pixel_size(self.config.icon_size);
            btn.set_child(Some(&img));
        } else {
            btn.set_label(&app.name);
        }

        let app_clone = app.clone();
        btn.connect_clicked(move |_| {
            app_clone.launch();
        });

        self.box_container.append(&btn);
    }
}
