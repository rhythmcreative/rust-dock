mod cli;
mod config;
mod dock;
mod hyprland_handler;
mod app_info;
mod style;

use gtk4::prelude::*;
use gtk4::Application;
use clap::Parser;
use cli::Cli;
use config::Config;
use dock::Dock;
use hyprland_handler::{DockEvent, invalidate_client_cache, invalidate_gaps_cache};
use std::rc::Rc;
use std::cell::{RefCell, Cell};
use std::time::Duration;

/// Remove stale window-preview screenshots and recreate the work directory.
fn cleanup_preview_tmp() {
    let dir = std::path::Path::new("/tmp/rust-dock");
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::create_dir_all(dir);
}

fn main() {
    env_logger::init();
    cleanup_preview_tmp();

    // CLI options override the config file.
    let cli = Cli::parse();

    if cli.toggle {
        let my_pid = std::process::id();
        let mut sent = false;
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
                if pid == my_pid { continue; }
                let comm = std::fs::read_to_string(format!("/proc/{}/comm", pid))
                    .unwrap_or_default();
                if comm.trim() == "rust-dock" {
                    let _ = std::process::Command::new("kill")
                        .args(["-USR1", &pid.to_string()])
                        .status();
                    sent = true;
                }
            }
        }
        std::process::exit(if sent { 0 } else { 1 });
    }

    let mut config = Config::new();
    cli.apply_to(&mut config);

    // Silence spurious GTK internal hover warnings when clearing/rebuilding widgets
    glib::log_set_writer_func(|level, fields| {
        let mut is_ancestor_warning = false;
        for field in fields {
            if field.key() == "MESSAGE"
                && let Some(msg) = field.value_str()
                    && msg.contains("gtk_widget_is_ancestor") {
                        is_ancestor_warning = true;
                        break;
                    }
        }

        if is_ancestor_warning {
            glib::LogWriterOutput::Handled
        } else {
            glib::log_writer_default(level, fields)
        }
    });

    let config_rc = Rc::new(RefCell::new(config));
    let cli_rc = Rc::new(cli);

    // Make the GTK application ID unique per monitor so that multiple
    // instances (one per output) can run simultaneously without GTK
    // redirecting the second launch to the already-running instance.
    let app_id = match config_rc.borrow().output.as_deref() {
        Some(output) => {
            // Sanitize: GTK app IDs may only contain [A-Za-z0-9._-]
            let safe: String = output.chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
                .collect();
            format!("com.github.rhythmcreative.rust_dock.{}", safe)
        }
        None => "com.github.rhythmcreative.rust_dock".to_string(),
    };

    let app = Application::builder()
        .application_id(&app_id)
        .build();

    let config_activate = Rc::clone(&config_rc);
    let cli_activate = Rc::clone(&cli_rc);
    let dock_instance: Rc<RefCell<Option<Rc<Dock>>>> = Rc::new(RefCell::new(None));
    let dock_instance_activate = Rc::clone(&dock_instance);

    app.connect_activate(move |app| {
        if let Some(existing_dock) = dock_instance_activate.borrow().as_ref() {
            existing_dock.set_dock_visible(true);
            existing_dock.refresh();
            return;
        }

        // Register Flatpak (and other XDG) icon directories with GTK's icon theme
        // so that app icons installed via Flatpak are resolved correctly.
        if let Some(display) = gtk4::gdk::Display::default() {
            let theme = gtk4::IconTheme::for_display(&display);
            theme.add_search_path("/var/lib/flatpak/exports/share/icons");
            if let Some(data_dir) = dirs::data_dir() {
                theme.add_search_path(data_dir.join("flatpak/exports/share/icons"));
            }
        }

        let config_ref = config_activate.borrow();
        style::load_css(&config_ref);
        drop(config_ref);

        let dock = Rc::new(Dock::new(app, Rc::clone(&config_activate)));
        dock.init();
        *dock_instance_activate.borrow_mut() = Some(Rc::clone(&dock));

        // --- Hyprland socket listener (0ms latency, zero polling) ---
        let (tx_hypr, rx_hypr) = async_channel::unbounded::<DockEvent>();
        hyprland_handler::start_listener(move |ev| {
            let _ = tx_hypr.send_blocking(ev);
        });

        let dock_hypr = Rc::clone(&dock);
        let pending_refresh = Rc::new(Cell::new(false));
        let pr = Rc::clone(&pending_refresh);
        let dh = Rc::clone(&dock_hypr);

        glib::MainContext::default().spawn_local(async move {
            while let Ok(ev) = rx_hypr.recv().await {
                match ev {
                    DockEvent::WindowList => {
                        invalidate_client_cache();
                        dock_hypr.refresh_workspaces();
                        if !pr.get() {
                            pr.set(true);
                            let pr_c = Rc::clone(&pr);
                            let dh_c = Rc::clone(&dh);
                            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                                pr_c.set(false);
                                invalidate_client_cache();
                                dh_c.refresh();
                            });
                        }
                    }
                    DockEvent::Workspace => {
                        dock_hypr.refresh_workspaces();
                    }
                    DockEvent::Focus(class) => {
                        dock_hypr.update_active(&class);
                    }
                    DockEvent::ConfigReloaded => {
                        invalidate_gaps_cache();
                    }
                }
            }
        });

        // --- Pywal file watcher (Event-driven, 0 polling) ---
        let (tx_pywal, rx_pywal) = async_channel::unbounded::<()>();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut wal_dir) = dirs::cache_dir() {
                wal_dir.push("wal");
                while !wal_dir.exists() {
                    std::thread::sleep(Duration::from_secs(5));
                }
                let (tx, rx) = std::sync::mpsc::channel();
                if let Ok(mut watcher) = notify::recommended_watcher(tx) {
                    let _ = watcher.watch(&wal_dir, RecursiveMode::NonRecursive);
                    for e in rx.into_iter().flatten() {
                        let is_css = e.paths.iter().any(|p| {
                            p.extension().is_some_and(|ext| ext == "css")
                        });
                        if is_css && (e.kind.is_modify() || e.kind.is_create()) {
                            let _ = tx_pywal.send_blocking(());
                        }
                    }
                }
            }
        });

        let dock_pywal = Rc::clone(&dock);
        let config_pywal = Rc::clone(&config_activate);
        let pywal_debouncing = Rc::new(Cell::new(false));
        glib::MainContext::default().spawn_local(async move {
            while let Ok(()) = rx_pywal.recv().await {
                if !pywal_debouncing.get() {
                    pywal_debouncing.set(true);
                    let deb = Rc::clone(&pywal_debouncing);
                    let dock = Rc::clone(&dock_pywal);
                    let config = Rc::clone(&config_pywal);
                    glib::timeout_add_local_once(Duration::from_millis(60), move || {
                        deb.set(false);
                        let cfg = config.borrow();
                        style::load_css(&cfg);
                        drop(cfg);
                        dock.refresh();
                    });
                }
            }
        });

        // --- Config file hot-reload (Event-driven, 0 polling) ---
        let (tx_cfg, rx_cfg) = async_channel::unbounded::<()>();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut conf_dir) = dirs::config_dir() {
                conf_dir.push("rust-dock");
                let (tx, rx) = std::sync::mpsc::channel();
                if let Ok(mut watcher) = notify::recommended_watcher(tx)
                    && conf_dir.exists() {
                        let _ = watcher.watch(&conf_dir, RecursiveMode::Recursive);
                        for event in rx {
                            if let Ok(e) = event
                                && (e.kind.is_modify() || e.kind.is_create()) {
                                    let _ = tx_cfg.send_blocking(());
                                }
                        }
                    }
            }
        });

        let dock_cfg = Rc::clone(&dock);
        let config_cfg = Rc::clone(&config_activate);
        let cli_cfg = Rc::clone(&cli_activate);
        let cfg_debouncing = Rc::new(Cell::new(false));
        glib::MainContext::default().spawn_local(async move {
            while let Ok(()) = rx_cfg.recv().await {
                if !cfg_debouncing.get() {
                    cfg_debouncing.set(true);
                    let deb = Rc::clone(&cfg_debouncing);
                    let dock = Rc::clone(&dock_cfg);
                    let config = Rc::clone(&config_cfg);
                    let cli = Rc::clone(&cli_cfg);
                    glib::timeout_add_local_once(Duration::from_millis(100), move || {
                        deb.set(false);
                        let mut new_cfg = Config::new();
                        cli.apply_to(&mut new_cfg);
                        *config.borrow_mut() = new_cfg;
                        let cfg = config.borrow();
                        style::load_css(&cfg);
                        drop(cfg);
                        dock.refresh();
                    });
                }
            }
        });

        // --- Signal handler (SIGHUP/SIGUSR1 toggle, SIGUSR2 show, SIGTERM/INT quit) ---
        let (tx_sig, rx_sig) = async_channel::unbounded::<i32>();
        std::thread::spawn(move || {
            use signal_hook::iterator::Signals;
            if let Ok(mut signals) = Signals::new([
                signal_hook::consts::SIGUSR1,
                signal_hook::consts::SIGUSR2,
                signal_hook::consts::SIGHUP,
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGINT,
            ]) {
                for signal in signals.forever() {
                    let _ = tx_sig.send_blocking(signal);
                }
            }
        });

        let dock_signal = Rc::clone(&dock);
        let app_quit = app.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(sig) = rx_sig.recv().await {
                if sig == signal_hook::consts::SIGUSR1 || sig == signal_hook::consts::SIGHUP {
                    dock_signal.toggle_visibility();
                } else if sig == signal_hook::consts::SIGUSR2 {
                    dock_signal.set_dock_visible(true);
                } else {
                    let _ = std::fs::remove_dir_all("/tmp/rust-dock");
                    app_quit.quit();
                }
            }
        });
    });

    app.run_with_args::<&str>(&[]);
}
