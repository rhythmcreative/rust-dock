use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, Instant};

const CLIENT_CACHE_TTL: Duration = Duration::from_millis(120);

thread_local! {
    static GAPS_OUT: Cell<Option<i32>> = const { Cell::new(None) };
    /// Short-lived cache for `hyprctl clients -j`. Invalidated on window events
    /// and automatically expired after CLIENT_CACHE_TTL.
    static CLIENT_CACHE: RefCell<Option<(Instant, Vec<HyprClient>)>> = const { RefCell::new(None) };
}

/// Discard the client list cache so the next call to `get_clients()` re-fetches.
/// Call this whenever a window open/close event is received.
pub fn invalidate_client_cache() {
    CLIENT_CACHE.with(|c| *c.borrow_mut() = None);
}

/// Discard the cached gaps_out value so the next call re-fetches from Hyprland.
/// Call this when Hyprland's config is reloaded.
pub fn invalidate_gaps_cache() {
    GAPS_OUT.with(|c| c.set(None));
}

/// Locates the Hyprland command socket (.socket.sock) without requiring hyprctl.
pub fn hypr_socket_path() -> Option<String> {
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        dirs::runtime_dir()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/tmp".to_string())
    });

    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().or_else(|| {
        let path = std::path::PathBuf::from(&xdg_runtime).join("hypr");
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().join(".socket.sock").exists() {
                    return Some(name);
                }
            }
        }
        None
    })?;

    let candidates = [
        format!("{}/hypr/{}/.socket.sock", xdg_runtime, sig),
        format!("/tmp/hypr/{}/.socket.sock", sig),
    ];

    candidates.into_iter().find(|p| std::path::Path::new(p).exists())
}

/// Run a `hyprctl` command as a fallback for when the IPC socket is unreachable.
///
/// Always wrapped in `timeout` so a wedged Hyprland can't block the dock's main
/// loop forever — `Command::output()` alone has no deadline and would hang the
/// whole GTK thread. Prefer `hypr_socket_request` everywhere.
pub fn hyprctl(args: &[&str]) -> Option<std::process::Output> {
    const FALLBACK_TIMEOUT_SECS: &str = "1";
    let out = Command::new("timeout")
        .arg(FALLBACK_TIMEOUT_SECS)
        .arg("hyprctl")
        .args(args)
        .output()
        .ok()?;
    // `timeout` exits 124 when it had to kill the child.
    if !out.status.success() { return None; }
    // hyprctl reports failures with exit code 0 and an "unknown request" /
    // "error" line on stdout, so the status alone is not a success signal.
    if hyprctl_reported_error(&out) { return None; }
    Some(out)
}

/// True when a hyprctl invocation that exited 0 actually reported an error.
///
/// Hyprland's CLI is inconsistent here: `hyprctl bogus` prints "unknown
/// request" and still exits 0, so trusting the status code silently turned
/// failed fallbacks into apparent successes.
fn hyprctl_reported_error(out: &std::process::Output) -> bool {
    const MARKERS: [&str; 5] = ["unknown request", "error", "invalid", "failed", "no such"];
    let stdout = String::from_utf8_lossy(&out.stdout);
    let text = format!("{}{}", stdout.to_lowercase(), String::from_utf8_lossy(&out.stderr).to_lowercase());
    // Every query this crate issues returns JSON. A payload that parses as a
    // JSON object/array is never an error response, and scanning it for
    // markers would misfire on arbitrary window titles like "Error Monitor".
    if stdout.trim_start().starts_with('{') || stdout.trim_start().starts_with('[') {
        return false;
    }
    MARKERS.iter().any(|m| text.contains(m))
}

/// Fire-and-forget `hyprctl` dispatch for older Hyprland versions that lack the
/// Lua dispatch API. Bounded by the same 1s timeout as `hyprctl`.
pub fn hyprctl_dispatch(lua: &str) -> bool {
    hyprctl(&["dispatch", lua]).is_some()
}

/// Non-blocking legacy dispatch, wrapped in `timeout` so the child can't outlive
/// the call if Hyprland is wedged.
fn hyprctl_dispatch_spawn(arg: &str) {
    let _ = Command::new("timeout")
        .args(["1", "hyprctl", "dispatch", arg])
        .spawn();
}

/// Send a direct command over Hyprland's command socket (.socket.sock).
/// 50x faster than spawning a `hyprctl` child process.
pub fn hypr_socket_request(cmd: &str) -> Option<Vec<u8>> {
    let path = hypr_socket_path()?;
    let mut stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    stream.set_write_timeout(Some(Duration::from_millis(500))).ok()?;
    stream.write_all(cmd.as_bytes()).ok()?;
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).ok()?;
    Some(buffer)
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
    pub floating: bool,
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
    /// Hyprland config was reloaded (e.g. `hyprctl reload`).
    ConfigReloaded,
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
    let floating = v.get("floating").and_then(|x| x.as_bool()).unwrap_or(false);
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
        floating,
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
        if let Some(buf) = hypr_socket_request("j/clients")
            && let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(&buf) {
                return values.iter().filter_map(client_from_value).collect();
            }
        let Some(out) = hyprctl(&["clients", "-j"]) else {
            return Vec::new();
        };
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
        if let Some(buf) = hypr_socket_request("j/getoption general:gaps_out")
            && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&buf)
                && let Some(custom) = json.get("custom").and_then(|v| v.as_str())
                    && let Some(first) = custom.split_whitespace().next()
                        && let Ok(val) = first.parse() {
                            return val;
                        }
        if let Some(out) = hyprctl(&["getoption", "general:gaps_out", "-j"])
            && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
                && let Some(custom) = json.get("custom").and_then(|v| v.as_str())
                    && let Some(first) = custom.split_whitespace().next() {
                        return first.parse().unwrap_or(0);
                    }
        0
    }

    /// Map a Wayland output connector name (e.g. "DP-1") to a Hyprland monitor index.
    pub fn get_monitor_id(&self, output_name: &str) -> Option<i32> {
        if let Some(buf) = hypr_socket_request("j/monitors")
            && let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&buf) {
                for m in &json {
                    if m.get("name").and_then(|v| v.as_str()) == Some(output_name) {
                        return m.get("id").and_then(|v| v.as_i64()).map(|id| id as i32);
                    }
                }
                return None;
            }
        let out = hyprctl(&["monitors", "-j"])?;
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
        if let Some(buf) = hypr_socket_request("j/activewindow")
            && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&buf) {
                return json.get("class")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_lowercase());
            }
        let out = hyprctl(&["activewindow", "-j"])?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        json.get("class")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
    }

}

// ── Hyprland dispatch helpers (0.56+ Lua API) ──────────────────────────────────
// Hyprland 0.56 switched to Lua: `hyprctl dispatch exec` now fails with
// `error: [string "return hl.dispatch(exec ..."]`. The new form is
// `hyprctl dispatch 'hl.dsp.exec_cmd("cmd")'` and similar `hl.dsp.*` calls.
// This module provides thin wrappers that try direct IPC socket and Lua dispatch first,
// falling back to hyprctl for older Hyprland versions.

fn lua_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Try the Hyprland Lua dispatch API over IPC, falling back to `hyprctl`
/// (1s-bounded) for older Hyprland versions. Returns true on success.
pub fn hypr_dispatch_lua_pub(lua: &str) -> bool {
    hypr_dispatch_lua(lua)
}

fn hypr_dispatch_lua(lua: &str) -> bool {
    let cmd = format!("dispatch {}", lua);
    if let Some(resp) = hypr_socket_request(&cmd) {
        let text = String::from_utf8_lossy(&resp).to_lowercase();
        if text.starts_with("ok") && !text.contains("error") {
            return true;
        }
    }
    // Fallback: `hyprctl dispatch <lua>` — same Lua form, so no per-version
    // branch needed here. Bounded by the 1s timeout inside `hyprctl`.
    hyprctl_dispatch(lua)
}

pub fn hypr_exec_cmd(cmd: &str) {
    let esc = lua_escape(cmd);
    let lua = format!("hl.dsp.exec_cmd(\"{}\")", esc);
    if hypr_dispatch_lua(&lua) {
        return;
    }
    // Fallback: direct shell spawn (works without Hyprland)
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}

pub fn hypr_focus_window(address: &str) {
    let lua = format!("hl.dsp.focus({{window=\"address:{}\"}})", address);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("focuswindow address:{}", address));
}

pub fn hypr_close_window(address: &str) {
    let lua = format!("hl.dsp.window.close({{window=\"address:{}\"}})", address);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("closewindow address:{}", address));
}

pub fn hypr_move_to_workspace(address: &str, ws_id: i32) {
    let lua = format!("hl.dsp.window.move({{workspace={}, window=\"address:{}\"}})", ws_id, address);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("movetoworkspacesilent {},address:{}", ws_id, address));
}

pub fn hypr_move_window_pixel(address: &str, x: i32, y: i32) {
    let lua = format!("hl.dsp.window.move({{x={}, y={}, window=\"address:{}\"}})", x, y, address);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("movewindowpixel exact {} {},address:{}", x, y, address));
}

pub fn hypr_resize_window_pixel(address: &str, w: i32, h: i32) {
    let lua = format!("hl.dsp.window.resize({{x={}, y={}, window=\"address:{}\"}})", w, h, address);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("resizewindowpixel exact {} {},address:{}", w, h, address));
}

pub fn hypr_focus_workspace(ws_id: i32) {
    let lua = format!("hl.dsp.focus({{workspace={}}})", ws_id);
    if hypr_dispatch_lua(&lua) { return; }
    hyprctl_dispatch_spawn(&format!("workspace {}", ws_id));
}

/// Scale passed to grim when capturing preview thumbnails.
/// Captures are 1/4 of the real window size, so anything measured in file
/// pixels has to be multiplied by 1/SCALE to match on-screen pixels.
const CAPTURE_SCALE: f32 = 0.25;

/// Corner radius used for the preview thumbnails, in on-screen pixels.
const THUMB_RADIUS_CSS: f32 = 16.0;

/// Round the corners of a PNG in place (corners become transparent).
///
/// GTK does not clip widget content (GtkPicture) to the CSS border-radius, so
/// live thumbnails stay square. Rounding them here is the only reliable way.
fn round_png_corners(path: &str, radius_px: f32) {
    let Ok(img) = image::open(path) else { return };
    let mut rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    if w < 4 || h < 4 {
        return;
    }
    let r = radius_px.max(1.0).min(w as f32 / 2.0).min(h as f32 / 2.0);
    let r = r as u32;
    let r2 = r as f32 * r as f32;

    // Only the four r×r corner boxes can contain clipped pixels; every other
    // pixel has dx or dy = -1 and is skipped by the original test. Walking the
    // full image did O(w·h) float math per capture for nothing, so this walks
    // just the corners. The geometry matches the original expression exactly:
    // on the leading edge dx = r - x - 1, on the trailing edge dx = x - (w - r).
    let corner = |rgba: &mut image::RgbaImage, x: u32, y: u32, dx: f32, dy: f32| {
        if dx >= 0.0 && dy >= 0.0 && dx * dx + dy * dy > r2 {
            rgba.get_pixel_mut(x, y).0 = [0, 0, 0, 0];
        }
    };

    for y in 0..r {
        for x in 0..r {
            // Leading edges: dx = r - x - 1 at pixel x.
            // Trailing edges: dx = (w - r) + x - (w - r) = x at pixel w-r+x,
            // so the trailing corner box starts at w-r, not w-1.
            corner(&mut rgba, x, y,
                   r as f32 - x as f32 - 1.0,
                   r as f32 - y as f32 - 1.0);
            corner(&mut rgba, w - r + x, y,
                   x as f32,
                   r as f32 - y as f32 - 1.0);
            corner(&mut rgba, x, h - r + y,
                   r as f32 - x as f32 - 1.0,
                   y as f32);
            corner(&mut rgba, w - r + x, h - r + y,
                   x as f32,
                   y as f32);
        }
    }
    let _ = rgba.save(path);
}

/// Round the thumbnail corners after a capture, translating the on-screen
/// radius into file pixels with the capture scale.
fn round_thumbnail_corners(path: &str) {
    round_png_corners(path, THUMB_RADIUS_CSS / CAPTURE_SCALE);
}

/// Cached metadata for one captured thumbnail.
#[derive(Debug, Clone, Copy)]
struct CaptureEntry {
    /// Window geometry the capture was taken at.
    at:     [i32; 2],
    size:   [i32; 2],
    /// When this capture was taken, for TTL expiry.
    captured_at: std::time::Instant,
}

/// Directory holding preview thumbnails.
const CAPTURE_DIR: &str = "/tmp/rust-dock";

thread_local! {
    /// Recent captures keyed by window address.
    ///
    /// `refresh()` rebuilds every widget, and the preview capture loop runs
    /// once per rebuild — so without this, hovering the dock re-forked grim and
    /// re-decoded/re-encoded the PNG for every window, repeatedly, with
    /// unchanged output.
    static CAPTURE_CACHE: RefCell<std::collections::HashMap<String, CaptureEntry>> =
        RefCell::new(std::collections::HashMap::new());
}

/// On-disk path of a window's thumbnail.
fn capture_path(address: &str) -> String {
    format!("{}/{}.png", CAPTURE_DIR, address.trim_start_matches("0x"))
}

/// How long a capture stays reusable. Long enough to absorb a burst of
/// rebuilds while the mouse crosses the dock, short enough that a moving
/// window still updates.
const CAPTURE_TTL: std::time::Duration = std::time::Duration::from_millis(1500);

/// Reusable capture for a window, if the geometry is unchanged and the PNG is
/// still on disk.
pub fn cached_capture(address: &str, at: [i32; 2], size: [i32; 2]) -> Option<String> {
    let path = capture_path(address);
    CAPTURE_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let usable = match cache.get(address) {
            Some(e) => {
                e.at == at && e.size == size && e.captured_at.elapsed() < CAPTURE_TTL
            }
            None => false,
        };
        if usable && std::path::Path::new(&path).exists() {
            Some(path)
        } else {
            // Geometry moved or the entry aged out: drop it so the next
            // successful capture replaces it rather than leaving it stale.
            cache.remove(address);
            None
        }
    })
}

/// Record a fresh capture so subsequent rebuilds can reuse it.
pub fn store_capture(address: &str, at: [i32; 2], size: [i32; 2]) {
    CAPTURE_CACHE.with(|c| {
        c.borrow_mut().insert(address.to_string(), CaptureEntry {
            at,
            size,
            captured_at: std::time::Instant::now(),
        });
    });
}

/// Forget a window's cache entry, e.g. once Hyprland reports it closed.
pub fn invalidate_capture(address: &str) {
    CAPTURE_CACHE.with(|c| {
        c.borrow_mut().remove(address);
    });
}

/// Drop every cached capture (e.g. on a global theme or scale change).
pub fn invalidate_all_captures() {
    CAPTURE_CACHE.with(|c| c.borrow_mut().clear());
}

/// Capture a screenshot of a specific window.
/// Uses grim with scale 0.25 (reduced resolution = fast), no PNG compression.
/// Falls back to geometry-based capture with the same scale.
/// Each attempt has a 3-second timeout to prevent hanging.
pub fn capture_window_screenshot(address: &str, stable_id: &Option<String>, at: [i32; 2], size: [i32; 2]) -> Option<String> {
    if size[0] <= 0 || size[1] <= 0 {
        return None;
    }

    let temp_path = capture_path(address);

    // Same window, same geometry, captured recently → reuse the PNG on disk
    // instead of forking grim and re-encoding the thumbnail.
    if let Some(existing) = cached_capture(address, at, size) {
        return Some(existing);
    }

    // Use scale 0.25 for fast captures at preview-appropriate resolution
    // PNG level 0 = no compression (fastest encode)
    if let Some(sid) = stable_id
        && !sid.is_empty() {
            let result = std::process::Command::new("timeout")
                .args(["3", "grim", "-s", "0.25", "-l", "0", "-t", "png", "-T", sid, &temp_path])
                .status()
                .ok();
            if let Some(status) = result
                && status.success() {
                    round_thumbnail_corners(&temp_path);
                    store_capture(address, at, size);
                    return Some(temp_path);
                }
        }

    let geometry = format!("{},{} {}x{}", at[0], at[1], size[0], size[1]);
    let result = std::process::Command::new("timeout")
        .args(["3", "grim", "-s", "0.25", "-l", "0", "-t", "png", "-g", &geometry, &temp_path])
        .status()
        .ok();

    if let Some(status) = result
        && status.success() {
            round_thumbnail_corners(&temp_path);
            store_capture(address, at, size);
            return Some(temp_path);
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
            let out = hyprctl(&["instanceinfo", "-j"]).ok_or("hyprctl instanceinfo unavailable")?;
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
            || line.starts_with("movewindow>>")
            || line.starts_with("movewindowv2>>")
        {
            // A moved window's cached thumbnail is stale; a closed one leaves a PNG
            // and a cache entry nothing will ever read again. Either way, drop it.
            if let Some(addr) = line.split(">>").nth(1).and_then(|r| r.split(',').next()) {
                invalidate_capture(addr);
            }
            on_event(DockEvent::WindowList);
        } else if line.starts_with("workspace>>")
            || line.starts_with("createworkspace>>")
            || line.starts_with("destroyworkspace>>")
            || line.starts_with("focusedmon>>")
        {
            on_event(DockEvent::Workspace);
        } else if line.starts_with("configreloaded>>") {
            // A config reload can change output scale, which alters capture
            // geometry for every window.
            invalidate_all_captures();
            on_event(DockEvent::ConfigReloaded);
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

    #[test]
    fn lua_escape_neutralizes_quotes_and_backslashes() {
        // An unescaped quote would break out of the Lua string and inject
        // arbitrary dispatch commands, so this must round-trip safely.
        assert_eq!(lua_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(lua_escape(r"a\b"), r"a\\b");
        assert_eq!(lua_escape("plain"), "plain");
    }

    #[test]
    fn hyprctl_reports_failure_as_none() {
        // hyprctl exits 0 on "unknown request", so status alone is a lie.
        assert!(hyprctl(&["definitely-not-a-real-subcommand"]).is_none());
    }

    fn fake_out(stdout: &str, stderr: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::Command::new("true").status().unwrap(),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn error_text_is_detected_despite_success_exit() {
        assert!(hyprctl_reported_error(&fake_out("unknown request", "")));
        assert!(hyprctl_reported_error(&fake_out("", "error: invalid command")));
        assert!(hyprctl_reported_error(&fake_out("failed to open socket", "")));
    }

    #[test]
    fn json_payload_with_error_word_is_not_mistaken_for_failure() {
        // A window genuinely titled "Error Monitor" must not make the whole
        // query look like it failed.
        let json = r#"[{"class":"emacs","title":"Error Monitor: buffer 1"}]"#;
        assert!(!hyprctl_reported_error(&fake_out(json, "")));
    }

    #[test]
    fn plain_json_is_never_an_error() {
        assert!(!hyprctl_reported_error(&fake_out(r#"{"id":0,"name":"DP-1"}"#, "")));
        assert!(!hyprctl_reported_error(&fake_out("[]", "")));
        assert!(!hyprctl_reported_error(&fake_out("", "")));
    }

    // ── Capture cache ────────────────────────────────────────────────────

    /// Creates the thumbnail file `cached_capture` looks for.
    fn fake_png(addr: &str) -> std::path::PathBuf {
        let path = std::path::PathBuf::from(capture_path(addr));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn cache_misses_when_nothing_was_stored() {
        invalidate_all_captures();
        assert!(cached_capture("0xdeadbeef", [0, 0], [100, 100]).is_none());
    }

    #[test]
    fn cache_hits_when_geometry_is_unchanged_and_file_exists() {
        let path = fake_png("cachehit");
        invalidate_all_captures();
        store_capture("cachehit", [10, 20], [800, 600]);
        let hit = cached_capture("cachehit", [10, 20], [800, 600]);
        let _ = std::fs::remove_file(&path);
        assert!(hit.is_some(), "unchanged geometry should reuse the capture");
        assert_eq!(hit.unwrap(), path.to_string_lossy());
    }

    #[test]
    fn cache_misses_when_the_window_moved_or_resized() {
        let path = fake_png("geomchange");
        invalidate_all_captures();
        store_capture("geomchange", [10, 20], [800, 600]);
        // Same address, different size: the old thumbnail is wrong.
        assert!(cached_capture("geomchange", [10, 20], [1024, 768]).is_none());
        // Same address, moved: a geometry-based capture is stale too.
        assert!(cached_capture("geomchange", [99, 99], [800, 600]).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cache_misses_when_the_png_disappeared() {
        let path = fake_png("vanishing");
        invalidate_all_captures();
        store_capture("vanishing", [0, 0], [100, 100]);
        assert!(cached_capture("vanishing", [0, 0], [100, 100]).is_some());
        std::fs::remove_file(&path).unwrap();
        // Entry is cached but the file is gone: must not report a hit.
        assert!(cached_capture("vanishing", [0, 0], [100, 100]).is_none());
    }

    #[test]
    fn invalidate_removes_a_single_entry() {
        let path = fake_png("single");
        invalidate_all_captures();
        store_capture("single", [1, 1], [100, 100]);
        assert!(cached_capture("single", [1, 1], [100, 100]).is_some());
        invalidate_capture("single");
        assert!(cached_capture("single", [1, 1], [100, 100]).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn capture_path_strips_the_0x_prefix() {
        assert!(capture_path("0xabc123").ends_with("/abc123.png"));
        assert!(!capture_path("0xabc123").contains("0x"));
    }

    // ── Corner rounding ──────────────────────────────────────────────────

    #[test]
    fn rounding_only_touches_the_four_corners() {
        use image::{Rgba, RgbaImage};
        let (w, h) = (200u32, 150u32);
        let mut img = RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]));
        let radius = 20.0_f32;

        // Reference implementation: the original full-image predicate.
        let mut expected = RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]));
        let r = radius as u32;
        let r2 = radius * radius;
        for y in 0..h {
            for x in 0..w {
                let dx = if x < r { radius - x as f32 - 1.0 }
                         else if x as f32 >= w as f32 - radius { x as f32 - (w as f32 - radius) }
                         else { -1.0 };
                let dy = if y < r { radius - y as f32 - 1.0 }
                         else if y as f32 >= h as f32 - radius { y as f32 - (h as f32 - radius) }
                         else { -1.0 };
                if dx >= 0.0 && dy >= 0.0 && dx * dx + dy * dy > r2 {
                    expected.get_pixel_mut(x, y).0 = [0, 0, 0, 0];
                }
            }
        }

        // New implementation, same geometry, only the corner boxes.
        let rr = r as f32;
        let r2n = rr * rr;
        let corner = |rgba: &mut RgbaImage, x: u32, y: u32, dx: f32, dy: f32| {
            if dx >= 0.0 && dy >= 0.0 && dx * dx + dy * dy > r2n {
                rgba.get_pixel_mut(x, y).0 = [0, 0, 0, 0];
            }
        };
        for y in 0..r {
            for x in 0..r {
                corner(&mut img, x, y, rr - x as f32 - 1.0, rr - y as f32 - 1.0);
                corner(&mut img, w - r + x, y, x as f32, rr - y as f32 - 1.0);
                corner(&mut img, x, h - r + y, rr - x as f32 - 1.0, y as f32);
                corner(&mut img, w - r + x, h - r + y, x as f32, y as f32);
            }
        }

        // They must agree pixel for pixel, or thumbnails change appearance.
        assert_eq!(img.as_raw(), expected.as_raw(),
            "corner-only rounding diverged from the original full-image version");
    }
}
