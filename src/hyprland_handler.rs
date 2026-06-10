use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::time::{Duration, Instant};

const CLIENT_CACHE_TTL: Duration = Duration::from_millis(120);

thread_local! {
    static GAPS_OUT: Cell<Option<i32>> = const { Cell::new(None) };
    /// Short-lived cache for `hyprctl clients -j`. Invalidated on window events
    /// and automatically expired after CLIENT_CACHE_TTL.
    static CLIENT_CACHE: RefCell<Option<(Instant, Vec<HyprClient>)>> = RefCell::new(None);
}

/// Discard the client list cache so the next call to `get_clients()` re-fetches.
/// Call this whenever a window open/close event is received.
pub fn invalidate_client_cache() {
    CLIENT_CACHE.with(|c| *c.borrow_mut() = None);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprClient {
    pub address: String,
    #[serde(alias = "stableId")]
    pub stable_id: Option<String>,
    pub class: String,
    pub title: String,
    pub pid: i32,
    pub workspace: HyprWorkspace,
    /// Hyprland monitor index (0-based). -1 if unknown.
    pub monitor: i32,
    pub at: [i32; 2],
    pub size: [i32; 2],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprWorkspace {
    pub id: i32,
    pub name: String,
}

pub struct HyprlandHandler {}

/// What changed in Hyprland, so the main loop can react cheaply.
#[derive(Debug, Clone)]
pub enum DockEvent {
    /// A window opened or closed — the dock's app list may have changed.
    WindowList,
    /// Focus moved to a window of the given (lowercased) class.
    Focus(String),
    /// Active workspace changed or workspaces were created/destroyed.
    Workspace,
}

/// A Hyprland workspace with its window count.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HyprWorkspaceInfo {
    pub id:      i32,
    pub name:    String,
    pub windows: u32,
}

/// Extract the focused window class from an `activewindow>>CLASS,TITLE` event line.
pub fn parse_active_class(line: &str) -> Option<String> {
    line.strip_prefix("activewindow>>")
        .map(|rest| rest.split(',').next().unwrap_or("").trim().to_lowercase())
}

/// Read a `[i32; 2]` from a JSON array, defaulting to zeros on anything unexpected.
fn arr2(v: Option<&serde_json::Value>) -> [i32; 2] {
    let Some(a) = v.and_then(|x| x.as_array()) else { return [0, 0] };
    let get = |i: usize| a.get(i).and_then(|n| n.as_i64()).unwrap_or(0) as i32;
    [get(0), get(1)]
}

/// Build a `HyprClient` from a raw JSON object, tolerating missing/odd fields.
/// Only `address` is required; everything else falls back to a sane default.
fn client_from_value(v: &serde_json::Value) -> Option<HyprClient> {
    let address = v.get("address")?.as_str()?.to_string();
    let class = v.get("class").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let pid = v.get("pid").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    // stableId may be a string or a number depending on the Hyprland version.
    let stable_id = v.get("stableId").map(|x| match x {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    });
    let workspace = v.get("workspace");
    let workspace = HyprWorkspace {
        id: workspace.and_then(|w| w.get("id")).and_then(|x| x.as_i64()).unwrap_or(0) as i32,
        name: workspace.and_then(|w| w.get("name")).and_then(|x| x.as_str()).unwrap_or("").to_string(),
    };
    let monitor = v.get("monitor").and_then(|x| x.as_i64()).unwrap_or(-1) as i32;
    Some(HyprClient {
        address,
        stable_id,
        class,
        title,
        pid,
        workspace,
        monitor,
        at: arr2(v.get("at")),
        size: arr2(v.get("size")),
    })
}

impl HyprlandHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_clients(&self) -> Vec<HyprClient> {
        // Return cached result if still fresh.
        let cached = CLIENT_CACHE.with(|c| {
            c.borrow().as_ref().and_then(|(ts, list)| {
                if ts.elapsed() < CLIENT_CACHE_TTL { Some(list.clone()) } else { None }
            })
        });
        if let Some(list) = cached { return list; }

        let list = self.fetch_clients();
        CLIENT_CACHE.with(|c| *c.borrow_mut() = Some((Instant::now(), list.clone())));
        list
    }

    fn fetch_clients(&self) -> Vec<HyprClient> {
        let Ok(out) = Command::new("hyprctl").args(["clients", "-j"]).output() else {
            return Vec::new();
        };
        if !out.status.success() { return Vec::new(); }
        match serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            Ok(values) => values.iter().filter_map(client_from_value).collect(),
            Err(e) => { log::warn!("could not parse `hyprctl clients -j`: {e}"); Vec::new() }
        }
    }

    /// Get all open windows that belong to a specific app class.
    pub fn get_clients_for_class(&self, class: &str) -> Vec<HyprClient> {
        let class_lower = class.to_lowercase();
        let clients = self.get_clients();

        // Try exact match first
        let exact: Vec<HyprClient> = clients.iter()
            .filter(|c| c.class.to_lowercase() == class_lower)
            .cloned()
            .collect();
        if !exact.is_empty() {
            return exact;
        }

        // Fallback: find windows whose class starts with the requested class.
        // This handles cases like "brave" → "brave-browser" or "code" → "code-oss".
        // Minimum prefix length of 3 to avoid false positives (e.g. "b" matching "brave").
        if class_lower.len() >= 3 {
            let matched: Vec<HyprClient> = clients.into_iter()
                .filter(|c| c.class.to_lowercase().starts_with(&class_lower))
                .collect();
            if !matched.is_empty() {
                return matched;
            }
        }

        Vec::new()
    }

    pub fn get_gaps_out(&self) -> i32 {
        if let Some(cached) = GAPS_OUT.with(|c| c.get()) {
            return cached;
        }
        let value = self.query_gaps_out();
        GAPS_OUT.with(|c| c.set(Some(value)));
        value
    }

    fn query_gaps_out(&self) -> i32 {
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

    /// Map a Wayland output connector name (e.g. "DP-1") to a Hyprland monitor index.
    pub fn get_monitor_id(&self, output_name: &str) -> Option<i32> {
        let out = Command::new("hyprctl").args(["monitors", "-j"]).output().ok()?;
        if !out.status.success() { return None; }
        let json: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).ok()?;
        for m in &json {
            if m.get("name").and_then(|v| v.as_str()) == Some(output_name) {
                return m.get("id").and_then(|v| v.as_i64()).map(|id| id as i32);
            }
        }
        None
    }

    /// The class of the currently focused window, lowercased.
    pub fn get_active_class(&self) -> Option<String> {
        let out = Command::new("hyprctl").arg("activewindow").arg("-j").output().ok()?;
        if !out.status.success() { return None; }
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        json.get("class")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
    }

    /// The address of the currently focused window (e.g. "0x1234abcd").
    pub fn get_active_address(&self) -> Option<String> {
        let out = Command::new("hyprctl").arg("activewindow").arg("-j").output().ok()?;
        if !out.status.success() { return None; }
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        json.get("address")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// All open workspaces, sorted by ID.
    #[allow(dead_code)]
    pub fn get_workspaces(&self) -> Vec<HyprWorkspaceInfo> {
        let out = match Command::new("hyprctl").arg("workspaces").arg("-j").output() {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };
        let Ok(vals) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else {
            return Vec::new();
        };
        let mut list: Vec<HyprWorkspaceInfo> = vals.iter().filter_map(|v| {
            let id      = v.get("id")?.as_i64()? as i32;
            let name    = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let windows = v.get("windows").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            Some(HyprWorkspaceInfo { id, name, windows })
        }).collect();
        list.sort_by_key(|w| w.id);
        list
    }

    /// ID of the currently active workspace.
    #[allow(dead_code)]
    pub fn get_active_workspace_id(&self) -> i32 {
        let Ok(out) = Command::new("hyprctl").arg("activeworkspace").arg("-j").output() else {
            return 1;
        };
        if !out.status.success() { return 1; }
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
            return 1;
        };
        json.get("id").and_then(|v| v.as_i64()).unwrap_or(1) as i32
    }
}

/// Capture a screenshot of a specific window.
/// Uses grim with scale 0.25 (reduced resolution = fast), no PNG compression.
/// Falls back to geometry-based capture with the same scale.
/// Each attempt has a 5-second timeout to prevent hanging.
pub fn capture_window_screenshot(address: &str, stable_id: &Option<String>, at: [i32; 2], size: [i32; 2]) -> Option<String> {
    if size[0] <= 0 || size[1] <= 0 {
        return None;
    }

    let clean_addr = address.trim_start_matches("0x");
    let temp_path = format!("/tmp/rust-dock-preview-{}.png", clean_addr);

    // Use scale 0.25 for fast captures at preview-appropriate resolution
    // PNG level 0 = no compression (fastest encode)
    if let Some(sid) = stable_id {
        if !sid.is_empty() {
            let result = std::process::Command::new("timeout")
                .args(["3", "grim", "-s", "0.25", "-l", "0", "-t", "png", "-T", sid, &temp_path])
                .status()
                .ok();
            if let Some(status) = result {
                if status.success() {
                    return Some(temp_path);
                }
            }
        }
    }

    let geometry = format!("{},{} {}x{}", at[0], at[1], size[0], size[1]);
    let result = std::process::Command::new("timeout")
        .args(["3", "grim", "-s", "0.25", "-l", "0", "-t", "png", "-g", &geometry, &temp_path])
        .status()
        .ok();

    if let Some(status) = result {
        if status.success() {
            return Some(temp_path);
        }
    }

    None
}

/// Connects directly to the Hyprland IPC socket and listens for window events.
/// Calls `on_event` whenever a window opens, closes, moves, or focus changes.
pub fn start_listener<F>(on_event: F)
where
    F: Fn(DockEvent) + Send + 'static,
{
    std::thread::spawn(move || {
        loop {
            if let Err(e) = run_listener(&on_event) {
                log::warn!("Hyprland listener error: {e}, reconnecting in 1s...");
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

fn run_listener<F>(on_event: &F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(DockEvent),
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

    log::debug!("Connecting to Hyprland socket: {socket_path}");

    let stream = UnixStream::connect(&socket_path)?;
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        let line = line?;
        // Window open/close changes the app list and needs a full rebuild.
        // Focus changes only update the active highlight (cheap). Move/workspace
        // events fire constantly and are ignored.
        if line.starts_with("openwindow>>")
            || line.starts_with("openwindowv2>>")
            || line.starts_with("closewindow>>")
        {
            on_event(DockEvent::WindowList);
        } else if line.starts_with("workspace>>")
            || line.starts_with("createworkspace>>")
            || line.starts_with("destroyworkspace>>")
            || line.starts_with("focusedmon>>")
        {
            on_event(DockEvent::Workspace);
        } else if let Some(class) = parse_active_class(&line) {
            on_event(DockEvent::Focus(class));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_active_class() {
        assert_eq!(parse_active_class("activewindow>>kitty,~/work").as_deref(), Some("kitty"));
        assert_eq!(parse_active_class("activewindow>>Brave-browser,Title").as_deref(), Some("brave-browser"));
        assert_eq!(parse_active_class("activewindow>>,").as_deref(), Some(""));
    }

    #[test]
    fn ignores_non_focus_lines() {
        assert_eq!(parse_active_class("openwindow>>0x1,2,kitty,title"), None);
        assert_eq!(parse_active_class("workspace>>2"), None);
    }
}
