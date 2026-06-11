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
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

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
            if field.key() == "MESSAGE" {
                if let Some(msg) = field.value_str() {
                    if msg.contains("gtk_widget_is_ancestor") {
                        is_ancestor_warning = true;
                        break;
                    }
                }
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
    app.connect_activate(move |app| {
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
        style::load_css(&*config_ref);
        drop(config_ref);

        let dock = Rc::new(Dock::new(app, Rc::clone(&config_activate)));
        dock.init();

        // --- Hyprland socket listener ---
        let (tx_hypr, rx_hypr) = std::sync::mpsc::channel::<DockEvent>();
        hyprland_handler::start_listener(move |ev| {
            let _ = tx_hypr.send(ev);
        });

        // Poll at 50ms. Window open/close rebuilds the dock; focus changes only
        // update the active highlight; workspace changes refresh the ws bar.
        let dock_hypr = Rc::clone(&dock);
        let rx_hypr = Arc::new(Mutex::new(rx_hypr));
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let (window_changed, workspace_changed, last_focus, cfg_reloaded) = {
                let Ok(rx) = rx_hypr.lock() else { return glib::ControlFlow::Continue; };
                let mut changed = false;
                let mut ws_changed = false;
                let mut focus = None;
                let mut cfg_reload = false;
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        DockEvent::WindowList       => changed = true,
                        DockEvent::Workspace        => ws_changed = true,
                        DockEvent::Focus(class)     => focus = Some(class),
                        DockEvent::ConfigReloaded   => cfg_reload = true,
                    }
                }
                (changed, ws_changed, focus, cfg_reload)
            };
            if window_changed {
                // Discard the client cache so both rebuilds fetch fresh data.
                invalidate_client_cache();
                // Only the workspace bar (running apps) needs updating for most
                // open/close events — pinned icons and layout stay the same.
                dock_hypr.refresh_workspaces();
                // Race condition fix: Hyprland may emit `openwindow` before
                // `hyprctl clients -j` lists the new window. Full refresh 350ms
                // later guarantees a new pinned-app indicator is picked up too.
                let dock_delayed = Rc::clone(&dock_hypr);
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(350),
                    move || {
                        invalidate_client_cache();
                        dock_delayed.refresh();
                    },
                );
            }
            if workspace_changed && !window_changed {
                dock_hypr.refresh_workspaces();
            }
            if let Some(class) = last_focus {
                dock_hypr.update_active(&class);
            }
            if cfg_reloaded {
                invalidate_gaps_cache();
            }
            glib::ControlFlow::Continue
        });

        // --- Pywal file watcher ---
        // Watch the whole ~/.cache/wal/ directory so we catch file recreations
        // (pywal deletes and rewrites files, which inotify sees as CREATE, not MODIFY).
        let (tx_pywal, rx_pywal) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut wal_dir) = dirs::cache_dir() {
                wal_dir.push("wal");
                // If the directory doesn't exist yet, poll until pywal runs for the first time.
                while !wal_dir.exists() {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                let (tx, rx) = std::sync::mpsc::channel();
                if let Ok(mut watcher) = notify::recommended_watcher(tx) {
                    let _ = watcher.watch(&wal_dir, RecursiveMode::NonRecursive);
                    for event in rx {
                        if let Ok(e) = event {
                            let is_css = e.paths.iter().any(|p| {
                                p.extension().map_or(false, |ext| ext == "css")
                            });
                            if is_css && (e.kind.is_modify() || e.kind.is_create()) {
                                let _ = tx_pywal.send(());
                            }
                        }
                    }
                }
            }
        });

        let dock_pywal = Rc::clone(&dock);
        let config_pywal = Rc::clone(&config_activate);
        let rx_pywal = Arc::new(Mutex::new(rx_pywal));
        // Seed the mtime so the first poll doesn't trigger a spurious reload.
        style::pywal_file_changed();
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            // Primary: inotify watcher. Fallback: mtime polling (catches atomic
            // renames and directory recreations that inotify may miss).
            let notified = {
                let Ok(rx) = rx_pywal.lock() else { return glib::ControlFlow::Continue; };
                rx.try_recv().is_ok()
            };
            if notified || style::pywal_file_changed() {
                let cfg = config_pywal.borrow();
                style::load_css(&*cfg);
                drop(cfg);
                dock_pywal.refresh();
            }
            glib::ControlFlow::Continue
        });

        // --- Config file hot-reload ---
        let (tx_cfg, rx_cfg) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};
            if let Some(mut conf_dir) = dirs::config_dir() {
                conf_dir.push("rust-dock");
                let (tx, rx) = std::sync::mpsc::channel();
                if let Ok(mut watcher) = notify::recommended_watcher(tx) {
                    if conf_dir.exists() {
                        let _ = watcher.watch(&conf_dir, RecursiveMode::Recursive);
                        for event in rx {
                            if let Ok(e) = event {
                                if e.kind.is_modify() || e.kind.is_create() {
                                    let _ = tx_cfg.send(());
                                }
                            }
                        }
                    }
                }
            }
        });

        let dock_cfg = Rc::clone(&dock);
        let config_cfg = Rc::clone(&config_activate);
        let cli_cfg = Rc::clone(&cli_activate);
        let rx_cfg = Arc::new(Mutex::new(rx_cfg));
        glib::timeout_add_local(std::time::Duration::from_millis(700), move || {
            let changed = {
                let Ok(rx) = rx_cfg.lock() else { return glib::ControlFlow::Continue; };
                let mut c = false;
                while rx.try_recv().is_ok() { c = true; }
                c
            };
            if changed {
                // Re-read the file, then re-apply CLI overrides (they win).
                let mut new_cfg = Config::new();
                cli_cfg.apply_to(&mut new_cfg);
                *config_cfg.borrow_mut() = new_cfg;
                let cfg = config_cfg.borrow();
                style::load_css(&*cfg);
                drop(cfg);
                dock_cfg.refresh();
            }
            glib::ControlFlow::Continue
        });

        // --- SIGUSR1/SIGUSR2 signal handler ---
        let (tx_sig, rx_sig) = std::sync::mpsc::channel::<i32>();
        std::thread::spawn(move || {
            use signal_hook::iterator::Signals;
            if let Ok(mut signals) = Signals::new(&[
                signal_hook::consts::SIGUSR1,
                signal_hook::consts::SIGUSR2,
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGINT,
            ]) {
                for signal in signals.forever() {
                    let _ = tx_sig.send(signal);
                }
            }
        });

        let dock_signal = Rc::clone(&dock);
        let app_quit = app.clone();
        let rx_sig = Arc::new(Mutex::new(rx_sig));
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            let Ok(rx) = rx_sig.lock() else { return glib::ControlFlow::Continue; };
            if let Ok(sig) = rx.try_recv() {
                if sig == signal_hook::consts::SIGUSR1 {
                    dock_signal.toggle_visibility();
                } else if sig == signal_hook::consts::SIGUSR2 {
                    dock_signal.set_dock_visible(true);
                } else {
                    // SIGTERM / SIGINT: clean up screenshots and exit gracefully.
                    let _ = std::fs::remove_dir_all("/tmp/rust-dock");
                    app_quit.quit();
                }
            }
            glib::ControlFlow::Continue
        });
    });

    app.run_with_args::<&str>(&[]);
}
