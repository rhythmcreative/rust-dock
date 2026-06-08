mod config;
mod dock;
mod hyprland_handler;
mod app_info;
mod style;

use gtk4::prelude::*;
use gtk4::Application;
use config::Config;
use dock::Dock;
use std::rc::Rc;

fn main() {
    env_logger::init();

    // Parse config first
    let config = Config::new();
    let config_rc = Rc::new(config);

    let app = Application::builder()
        .application_id("com.github.rhythmcreative.rust_dock")
        .build();

    let config_activate = Rc::clone(&config_rc);
    app.connect_activate(move |app| {
        let config = config_activate.as_ref();
        style::load_css(config);

        let dock = Rc::new(Dock::new(app, config.clone()));

        let (sender, receiver) = std::sync::mpsc::channel::<()>();

        let dock_clone = Rc::clone(&dock);
        let config_for_receiver = config.clone();
        glib::idle_add_local(move || {
            while let Ok(_) = receiver.try_recv() {
                style::load_css(&config_for_receiver);
                dock_clone.refresh();
            }
            glib::ControlFlow::Continue
        });

        let sender_clone = sender.clone();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut pywal_path) = dirs::cache_dir() {
                pywal_path.push("wal/colors-waybar.css");
                let (tx, rx) = std::sync::mpsc::channel();
                let mut watcher = notify::recommended_watcher(tx).unwrap();
                if pywal_path.exists() {
                    let _ = watcher.watch(&pywal_path, RecursiveMode::NonRecursive);
                    for _ in rx {
                        let _ = sender_clone.send(());
                    }
                }
            }
        });

        let dock_signal = Rc::clone(&dock);
        let (tx_sig, rx_sig) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use signal_hook::iterator::Signals;
            let mut signals = Signals::new(&[signal_hook::consts::SIGUSR1, signal_hook::consts::SIGUSR2]).unwrap();
            for signal in signals.forever() {
                let _ = tx_sig.send(signal);
            }
        });

        glib::idle_add_local(move || {
            if let Ok(sig) = rx_sig.try_recv() {
                if sig == signal_hook::consts::SIGUSR1 {
                    dock_signal.toggle_visibility();
                } else if sig == signal_hook::consts::SIGUSR2 {
                    dock_signal.set_dock_visible(true);
                }
            }
            glib::ControlFlow::Continue
        });

        let sender_hypr = sender.clone();
        hyprland_handler::start_listener(move || {
            let _ = sender_hypr.send(());
        });
    });

    app.run_with_args::<&str>(&[]);
}
