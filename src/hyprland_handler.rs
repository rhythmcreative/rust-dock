use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprClient {
    pub address: String,
    pub class: String,
    pub title: String,
    pub pid: i32,
    pub workspace: HyprWorkspace,
    pub at: [i32; 2],
    pub size: [i32; 2],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprWorkspace {
    pub id: i32,
    pub name: String,
}

pub struct HyprlandHandler {}

impl HyprlandHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_clients(&self) -> Vec<HyprClient> {
        let output = Command::new("hyprctl")
            .arg("clients")
            .arg("-j")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(clients) = serde_json::from_slice::<Vec<HyprClient>>(&out.stdout) {
                    return clients;
                }
            }
        }
        Vec::new()
    }

    /// Get all open windows that belong to a specific app class.
    pub fn get_clients_for_class(&self, class: &str) -> Vec<HyprClient> {
        let class_lower = class.to_lowercase();
        self.get_clients()
            .into_iter()
            .filter(|c| c.class.to_lowercase() == class_lower || c.class.to_lowercase().starts_with(&class_lower))
            .collect()
    }

    pub fn get_gaps_out(&self) -> i32 {
        let output = Command::new("hyprctl")
            .arg("getoption")
            .arg("general:gaps_out")
            .arg("-j")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(custom) = json.get("custom").and_then(|v| v.as_str()) {
                        if let Some(first) = custom.split_whitespace().next() {
                            return first.parse().unwrap_or(0);
                        }
                    }
                }
            }
        }
        0
    }
}

/// Capture a screenshot of a specific window region using grim.
/// Returns the path to the temporary PNG file on success.
pub fn capture_window_screenshot(address: &str, at: [i32; 2], size: [i32; 2]) -> Option<String> {
    if size[0] <= 0 || size[1] <= 0 {
        return None;
    }

    // Clean the address (remove 0x prefix for filename)
    let clean_addr = address.trim_start_matches("0x");
    let temp_path = format!("/tmp/rust-dock-preview-{}.png", clean_addr);

    let geometry = format!("{},{} {}x{}", at[0], at[1], size[0], size[1]);

    let status = Command::new("grim")
        .args(["-g", &geometry, "-t", "png", &temp_path])
        .status()
        .ok()?;

    if status.success() {
        Some(temp_path)
    } else {
        None
    }
}

/// Connects directly to the Hyprland IPC socket and listens for window events.
/// Calls `on_event` whenever a window opens, closes, moves, or focus changes.
pub fn start_listener<F>(on_event: F)
where
    F: Fn() + Send + 'static,
{
    std::thread::spawn(move || {
        loop {
            if let Err(e) = run_listener(&on_event) {
                eprintln!("[rust-dock] Hyprland listener error: {e}, reconnecting in 1s...");
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

fn run_listener<F>(on_event: &F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(),
{
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixStream;

    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .or_else(|_| {
            let out = Command::new("hyprctl").arg("instanceinfo").arg("-j").output()?;
            let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
            Ok::<String, Box<dyn std::error::Error>>(
                json["signature"].as_str().unwrap_or("").to_string()
            )
        })?;

    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());

    let candidates = vec![
        format!("{}/hypr/{}/.socket2.sock", xdg_runtime, sig),
        format!("/tmp/hypr/{}/.socket2.sock", sig),
    ];

    let socket_path = candidates
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| format!("Hyprland socket not found for sig={}", sig))?;

    eprintln!("[rust-dock] Connecting to Hyprland socket: {}", socket_path);

    let stream = UnixStream::connect(&socket_path)?;
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("openwindow>>")
            || line.starts_with("closewindow>>")
            || line.starts_with("movewindow>>")
            || line.starts_with("activewindow>>")
            || line.starts_with("activewindowv2>>")
            || line.starts_with("workspace>>")
            || line.starts_with("focusedmon>>")
            || line.starts_with("openwindowv2>>")
        {
            on_event();
        }
    }

    Ok(())
}
