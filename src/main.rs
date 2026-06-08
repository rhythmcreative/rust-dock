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
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

fn main() {
    env_logger::init();

    let config = Config::new();
    let config_rc = Rc::new(RefCell::new(config));

    let app = Application::builder()
        .application_id("com.github.rhythmcreative.rust_dock")
        .build();

    let config_activate = Rc::clone(&config_rc);
    app.connect_activate(move |app| {
        let config_ref = config_activate.borrow();
        style::load_css(&*config_ref);
        drop(config_ref);

        let dock = Rc::new(Dock::new(app, Rc::clone(&config_activate)));

        // --- Hyprland socket listener ---
        // tx is Send+Sync, rx stays on main thread wrapped in Arc<Mutex>
        let (tx_hypr, rx_hypr) = std::sync::mpsc::channel::<()>();

        hyprland_handler::start_listener(move || {
            let _ = tx_hypr.send(());
        });

        // Poll at 50ms for near-instant response to window changes
        let dock_hypr = Rc::clone(&dock);
        let config_hypr = Rc::clone(&config_activate);
        let rx_hypr = Arc::new(Mutex::new(rx_hypr));
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let rx = rx_hypr.lock().unwrap();
            let mut got_event = false;
            while rx.try_recv().is_ok() {
                got_event = true;
            }
            if got_event {
                let cfg = config_hypr.borrow();
                style::load_css(&*cfg);
                drop(cfg);
                dock_hypr.refresh();
            }
            glib::ControlFlow::Continue
        });

        // --- Pywal file watcher ---
        let (tx_pywal, rx_pywal) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut pywal_path) = dirs::cache_dir() {
                pywal_path.push("wal/colors-waybar.css");
                let (tx, rx) = std::sync::mpsc::channel();
                if let Ok(mut watcher) = notify::recommended_watcher(tx) {
                    if pywal_path.exists() {
                        let _ = watcher.watch(&pywal_path, RecursiveMode::NonRecursive);
                        for _ in rx {
                            let _ = tx_pywal.send(());
                        }
                    }
                }
            }
        });

        let dock_pywal = Rc::clone(&dock);
        let config_pywal = Rc::clone(&config_activate);
        let rx_pywal = Arc::new(Mutex::new(rx_pywal));
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let rx = rx_pywal.lock().unwrap();
            if rx.try_recv().is_ok() {
                let cfg = config_pywal.borrow();
                style::load_css(&*cfg);
                drop(cfg);
                dock_pywal.refresh();
            }
            glib::ControlFlow::Continue
        });

        // --- SIGUSR1/SIGUSR2 signal handler ---
        let (tx_sig, rx_sig) = std::sync::mpsc::channel::<i32>();
        std::thread::spawn(move || {
            use signal_hook::iterator::Signals;
            if let Ok(mut signals) = Signals::new(&[signal_hook::consts::SIGUSR1, signal_hook::consts::SIGUSR2]) {
                for signal in signals.forever() {
                    let _ = tx_sig.send(signal);
                }
            }
        });

        let dock_signal = Rc::clone(&dock);
        let rx_sig = Arc::new(Mutex::new(rx_sig));
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            let rx = rx_sig.lock().unwrap();
            if let Ok(sig) = rx.try_recv() {
                if sig == signal_hook::consts::SIGUSR1 {
                    dock_signal.toggle_visibility();
                } else if sig == signal_hook::consts::SIGUSR2 {
                    dock_signal.set_dock_visible(true);
                }
            }
            glib::ControlFlow::Continue
        });
    });

    app.run_with_args::<&str>(&[]);
}
